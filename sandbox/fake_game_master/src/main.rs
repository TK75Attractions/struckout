//! projector と touchpanel を単体でデバッグするための偽 game_master。
//!
//! 本物は MySQL を要求するので、描画やイベント処理を触るたびに
//! Docker を立ち上げるのは重すぎる。こちらは DB を持たず、
//! `GameMasterService` のうち対向が実際に使うところだけを喋る。
//!
//! ポートは本物と同じ `api/spec/servers.yaml` から取る (build.rs)。
//!
//! `sandbox/testTcpCLI` が偽 ball_tracker、こちらが偽 game_master。
//! 2 つ立てれば projector は本物の通信経路のまま動く。

use std::{
    collections::HashMap,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use parking_lot::Mutex;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, transport::Server};

use proto::{
    AddPlayerRequest, AddPlayerResponse, AddScoreRequest, AddScoreResponse, Difficulty,
    ListenEventsRequest, ListenEventsResponse, StartGameRequest, StartGameResponse,
    game_master_service_server::{GameMasterService, GameMasterServiceServer},
};

mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        "/tk75attractions.struckout.v1.rs"
    ));
}

/// 本物の GAME_DURATION (game_master/src/service.rs) に合わせてある。
const DEFAULT_DURATION_SECS: u64 = 150;

const DEFAULT_PORT: &str = env!("GAME_MASTER_GRPC_PORT");

// ---------------------------------------------------------------- 状態

/// 進行中のゲーム 1 つ。
struct RunningGame {
    machine_id: u32,
    difficulty: Difficulty,
    score: u32,
    /// 残り時間を刻んでいるタスク。手で終わらせるときに止める。
    timer: JoinHandle<()>,
}

/// ブロードキャストに流すイベント。proto の Event へはそのまま写せる。
#[derive(Clone, Debug)]
struct Ev {
    machine_id: u32,
    game_id: u32,
    data: proto::event::EventData,
}

impl From<Ev> for proto::Event {
    fn from(ev: Ev) -> Self {
        proto::Event {
            machine_id: ev.machine_id,
            game_id: ev.game_id,
            event_data: Some(ev.data),
        }
    }
}

struct Shared {
    /// 本物と同じく game_id で引ける。AddScore が NotFound を返す条件がこれ。
    running: Mutex<HashMap<u32, RunningGame>>,
    next_game_id: AtomicU32,
    next_player_id: AtomicU32,
    events: broadcast::Sender<Ev>,
}

impl Shared {
    fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(64);
        Arc::new(Self {
            running: Mutex::new(HashMap::new()),
            next_game_id: AtomicU32::new(1),
            next_player_id: AtomicU32::new(1),
            events,
        })
    }

    fn publish(&self, ev: Ev) {
        // 受信者が居なくても構わない。projector が繋ぐ前に start しても落とさない。
        let _ = self.events.send(ev);
    }

    /// ゲームを 1 つ始めて game_id を返す。
    fn start_game(self: &Arc<Self>, machine_id: u32, difficulty: Difficulty, secs: u64) -> u32 {
        let game_id = self.next_game_id.fetch_add(1, Ordering::Relaxed);

        let timer = tokio::spawn(run_timer(Arc::clone(self), game_id, machine_id, secs));

        self.running.lock().insert(
            game_id,
            RunningGame {
                machine_id,
                difficulty,
                score: 0,
                timer,
            },
        );

        self.publish(Ev {
            machine_id,
            game_id,
            data: proto::event::EventData::GameStarted(proto::event::GameStarted {
                difficulty: difficulty.into(),
            }),
        });

        say(&format!(
            "game {game_id} started on machine {machine_id} ({difficulty:?}, {secs}s)"
        ));
        game_id
    }

    /// ゲームを終わらせる。既に終わっていれば false。
    fn finish_game(&self, game_id: u32) -> bool {
        let Some(game) = self.running.lock().remove(&game_id) else {
            return false;
        };

        // 時間切れで呼ばれた場合、自分自身を abort しても既に走り終えている。
        game.timer.abort();

        self.publish(Ev {
            machine_id: game.machine_id,
            game_id,
            data: proto::event::EventData::GameFinished(proto::event::GameFinished {}),
        });

        say(&format!(
            "game {game_id} finished on machine {} with score {}",
            game.machine_id, game.score
        ));
        true
    }
}

/// 残り時間を毎秒流し、尽きたら終了する。本物の game_timer と同じ形。
async fn run_timer(shared: Arc<Shared>, game_id: u32, machine_id: u32, secs: u64) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    for elapsed in 0..secs {
        interval.tick().await;

        // 毎秒出すとログが読めなくなるので、送るだけで表示はしない。
        shared.publish(Ev {
            machine_id,
            game_id,
            data: proto::event::EventData::GameTimeLimitNotify(proto::event::GameTimeLimitNotify {
                remaining: Some(prost_types::Duration {
                    seconds: (secs - elapsed) as i64,
                    nanos: 0,
                }),
            }),
        });
    }

    shared.finish_game(game_id);
}

// ---------------------------------------------------------------- gRPC

#[derive(Clone)]
struct FakeService {
    shared: Arc<Shared>,
    default_duration_secs: u64,
}

type EventStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

/// ブロードキャストを受け取り、`keep` が通したものだけをストリームへ流す。
///
/// 受信が遅れて取りこぼした (Lagged) 場合は、その事実を伝えて次へ進む。
/// 本物はここで panic するが、デバッグ用にサーバごと落ちられては困る。
fn spawn_forwarder<T, F, M>(
    mut rx: broadcast::Receiver<Ev>,
    tx: mpsc::Sender<Result<T, Status>>,
    keep: F,
    wrap: M,
) where
    T: Send + 'static,
    F: Fn(&Ev) -> bool + Send + 'static,
    M: Fn(proto::Event) -> T + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if !keep(&ev) {
                        continue;
                    }
                    if tx.send(Ok(wrap(ev.into()))).await.is_err() {
                        return; // 相手が切れた
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    say(&format!("WARNING: a listener lagged and missed {n} event(s)"));
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

#[tonic::async_trait]
impl GameMasterService for FakeService {
    type StartGameStream = EventStream<StartGameResponse>;
    type ListenEventsStream = EventStream<ListenEventsResponse>;

    /// touchpanel が使う。ゲームを始めて、そのゲームのイベントだけを返す。
    async fn start_game(
        &self,
        req: Request<StartGameRequest>,
    ) -> Result<Response<Self::StartGameStream>, Status> {
        let req = req.into_inner();
        let difficulty: Difficulty = req
            .difficulty
            .try_into()
            .map_err(|e: prost::UnknownEnumValue| Status::invalid_argument(e.to_string()))?;

        let (tx, rx) = mpsc::channel(64);

        // GameStarted を流す前に購読しておく。順序が逆だと開始を取りこぼす。
        let event_rx = self.shared.events.subscribe();

        say(&format!(
            "StartGame from touchpanel: machine={} player={} {:?}",
            req.machine_id, req.player_id, difficulty
        ));

        let game_id =
            self.shared
                .start_game(req.machine_id, difficulty, self.default_duration_secs);

        spawn_forwarder(
            event_rx,
            tx,
            move |ev| ev.game_id == game_id,
            |event| StartGameResponse { event: Some(event) },
        );

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    /// projector が使う。号機で絞ったイベントを流し続ける。
    async fn listen_events(
        &self,
        req: Request<ListenEventsRequest>,
    ) -> Result<Response<Self::ListenEventsStream>, Status> {
        let remote = req
            .remote_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_owned());
        let machine_id = req.into_inner().machine_id;

        say(&format!(
            "ListenEvents from {remote} as machine {machine_id}"
        ));

        let (tx, rx) = mpsc::channel(64);
        spawn_forwarder(
            self.shared.events.subscribe(),
            tx,
            move |ev| ev.machine_id == machine_id,
            |event| ListenEventsResponse { event: Some(event) },
        );

        // 既に始まっているゲームがあれば、その事実を新しい購読者にも渡す。
        // 本物では touchpanel が start してから projector が繋ぐとは限らず、
        // 順序次第で開始を取りこぼすため、ここで追いつかせる。
        let catch_up = {
            let running = self.shared.running.lock();
            running
                .iter()
                .find(|(_, g)| g.machine_id == machine_id)
                .map(|(id, g)| (*id, g.difficulty))
        };

        if let Some((game_id, difficulty)) = catch_up {
            self.shared.publish(Ev {
                machine_id,
                game_id,
                data: proto::event::EventData::GameStarted(proto::event::GameStarted {
                    difficulty: difficulty.into(),
                }),
            });
            say(&format!(
                "replayed GameStarted for the running game {game_id}"
            ));
        }

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    /// projector が使う。得点は差分で届く。
    async fn add_score(
        &self,
        req: Request<AddScoreRequest>,
    ) -> Result<Response<AddScoreResponse>, Status> {
        let req = req.into_inner();

        let total = {
            let mut running = self.shared.running.lock();
            let Some(game) = running.get_mut(&req.game_id) else {
                // 本物と同じ。projector 側はこれを警告として出す。
                say(&format!(
                    "AddScore refused: game {} is not running",
                    req.game_id
                ));
                return Err(Status::not_found(format!(
                    "game with id {} does not exist or is not running",
                    req.game_id
                )));
            };
            game.score += req.score_to_add;
            game.score
        };

        say(&format!(
            "AddScore +{} on game {} (total {total})",
            req.score_to_add, req.game_id
        ));

        Ok(Response::new(AddScoreResponse {}))
    }

    /// DB が無いので通し番号を返すだけ。名前は覚えない。
    async fn add_player(
        &self,
        req: Request<AddPlayerRequest>,
    ) -> Result<Response<AddPlayerResponse>, Status> {
        let name = req.into_inner().name;
        let player_id = self.shared.next_player_id.fetch_add(1, Ordering::Relaxed);
        say(&format!("AddPlayer '{name}' -> {player_id}"));
        Ok(Response::new(AddPlayerResponse { player_id }))
    }
}

// ---------------------------------------------------------------- CLI

struct Options {
    port: u16,
    machine_id: u32,
    duration_secs: u64,
}

fn parse_options() -> Result<Option<Options>, String> {
    let mut port: u16 = DEFAULT_PORT
        .parse()
        .map_err(|_| format!("api/spec/servers.yaml has a bad port: {DEFAULT_PORT}"))?;
    let mut machine_id: u32 = 1;
    let mut duration_secs = DEFAULT_DURATION_SECS;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{arg} needs a value"))
        };

        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--port" => port = value()?.parse().map_err(|_| "--port must be a port number".to_owned())?,
            "--machine-id" => {
                machine_id = value()?
                    .parse()
                    .map_err(|_| "--machine-id must be a number".to_owned())?
            }
            "--duration" => {
                duration_secs = value()?
                    .parse()
                    .map_err(|_| "--duration must be seconds".to_owned())?
            }
            other => return Err(format!("unknown option '{other}'")),
        }
    }

    Ok(Some(Options {
        port,
        machine_id,
        duration_secs,
    }))
}

fn print_usage() {
    println!(
        "\
struckout fake game_master -- lets projector and touchpanel be debugged without
the real service (and without MySQL).

usage: fake_game_master [--port N] [--machine-id N] [--duration SECONDS]

  --port         gRPC port (default {DEFAULT_PORT}, from api/spec/servers.yaml)
  --machine-id   machine the interactive commands act on (default 1)
  --duration     game length in seconds (default {DEFAULT_DURATION_SECS})
"
    );
}

fn print_help(machine_id: u32) {
    println!(
        "\
  start [normal|hard|veryhard] [seconds]   start a game on machine {machine_id}
  finish [game_id]                         finish it early (no id = every game)
  status                                   show running games and listeners
  help                                     show this text
  exit                                     quit

The projector only reacts to GameStarted and GameFinished. Remaining time is sent
every second as well, so the stream is exercised even while nothing is printed.
"
    );
}

/// 出力は必ずここを通す。プロンプトの途中に割り込んでも読めるようにする。
fn say(message: &str) {
    println!("\r{message}");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = match parse_options() {
        Ok(Some(options)) => options,
        Ok(None) => {
            print_usage();
            return Ok(());
        }
        Err(message) => {
            eprintln!("ERROR: {message}");
            print_usage();
            std::process::exit(2);
        }
    };

    let shared = Shared::new();
    let service = FakeService {
        shared: Arc::clone(&shared),
        default_duration_secs: options.duration_secs,
    };

    let addr = format!("0.0.0.0:{}", options.port).parse()?;
    tokio::spawn(async move {
        if let Err(err) = Server::builder()
            .add_service(GameMasterServiceServer::new(service))
            .serve(addr)
            .await
        {
            eprintln!("ERROR: the gRPC server stopped: {err}");
            std::process::exit(1);
        }
    });

    say(&format!(
        "fake game_master listening on {addr} (h2c, no TLS), acting as machine {}",
        options.machine_id
    ));
    say("the projector connects with -masterPort <port> -networkMode real");
    print_help(options.machine_id);

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush()?;

        let Some(line) = lines.next_line().await? else {
            // スクリプトからコマンドを流し込むと、そこで stdin が閉じる。
            // そのたびにサーバまで落ちては対向として使えないので、
            // 以降は待つだけにする。
            say("stdin closed; still serving. press Ctrl-C to stop.");
            let _ = tokio::signal::ctrl_c().await;
            break;
        };

        // Windows でコマンドをパイプで流し込むと先頭に UTF-8 BOM が付くことがある。
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        match tokens[0].to_lowercase().as_str() {
            "start" => handle_start(&shared, &tokens, &options),
            "finish" => handle_finish(&shared, &tokens),
            "status" => handle_status(&shared),
            "help" => print_help(options.machine_id),
            "exit" | "quit" => break,
            other => say(&format!("unknown command '{other}'. type 'help'.")),
        }
    }

    say("bye");
    Ok(())
}

fn handle_start(shared: &Arc<Shared>, tokens: &[&str], options: &Options) {
    let difficulty = match tokens.get(1) {
        None => Difficulty::Normal,
        Some(name) => match name.to_lowercase().as_str() {
            "normal" => Difficulty::Normal,
            "hard" => Difficulty::Hard,
            "veryhard" => Difficulty::Veryhard,
            other => {
                say(&format!(
                    "unknown difficulty '{other}' (normal|hard|veryhard)"
                ));
                return;
            }
        },
    };

    let secs = match tokens.get(2) {
        None => options.duration_secs,
        Some(value) => match value.parse() {
            Ok(secs) => secs,
            Err(_) => {
                say(&format!("'{value}' is not a number of seconds"));
                return;
            }
        },
    };

    shared.start_game(options.machine_id, difficulty, secs);
}

fn handle_finish(shared: &Arc<Shared>, tokens: &[&str]) {
    match tokens.get(1) {
        Some(value) => match value.parse::<u32>() {
            Ok(game_id) => {
                if !shared.finish_game(game_id) {
                    say(&format!("game {game_id} is not running"));
                }
            }
            Err(_) => say(&format!("'{value}' is not a game id")),
        },
        None => {
            let ids: Vec<u32> = shared.running.lock().keys().copied().collect();
            if ids.is_empty() {
                say("no game is running");
                return;
            }
            for game_id in ids {
                shared.finish_game(game_id);
            }
        }
    }
}

fn handle_status(shared: &Arc<Shared>) {
    say(&format!("listeners : {}", shared.events.receiver_count()));

    let running = shared.running.lock();
    if running.is_empty() {
        say("games     : none running");
        return;
    }

    for (game_id, game) in running.iter() {
        say(&format!(
            "game {game_id}  : machine {} {:?} score {}",
            game.machine_id, game.difficulty, game.score
        ));
    }
}
