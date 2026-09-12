using UnityEngine;
using Struckout.Domain;

namespace Struckout.Unity
{
    /// <summary>
    /// 的 1 個の見た目。
    ///
    /// 当たったときは移動先へ滑らせるのではなく、その場で波紋を出しながら
    /// 縮んで消え、消え切ってから次の地点で波紋とともに膨らんで現れる。
    /// 途中の軌跡を見せないので、的が「別の場所に出た」ことが伝わりやすい。
    ///
    /// **演出の終わりを知らせるのはここの責任。** GameRuntime は Target を先に
    /// 動かしてから MoveTo を呼び、あわせて当たり判定から外す
    /// (<see cref="Target.BeginRelocation"/>)。そのままだと縮んで消えている間
    /// ずっと「何も描かれていない場所」に判定があることになるため。
    /// 膨らみ切ったところで <see cref="Target.EndRelocation"/> を呼んで戻す。
    ///
    /// 演出の長さを知っているのは描画側だけなので、秒数を Application 層にも
    /// 置いて二重管理にはしていない。
    /// </summary>
    [RequireComponent(typeof(SpriteRenderer))]
    public class CircleTargetUI : MonoBehaviour, ITargetUI
    {
        private enum Motion
        {
            /// <summary>通常表示。</summary>
            Idle,

            /// <summary>当たった場所で縮んでいる最中。</summary>
            Collapsing,

            /// <summary>移動先で膨らんでいる最中。</summary>
            Expanding,
        }

        [Header("Hit animation")]
        [SerializeField]
        [Tooltip("当たった場所で縮んで消えるまでの秒数。")]
        private float _collapseSeconds = 0.18f;

        [SerializeField]
        [Tooltip("移動先で膨らんで現れるまでの秒数。")]
        private float _expandSeconds = 0.22f;

        [SerializeField]
        [Tooltip("波紋の明るさ。0 にすると波紋を出さず、縮小と拡大だけになる。")]
        private float _rippleStrength = 1f;

        /// <summary>NeonRing シェーダが持つ、的の境界を表す UV 半径。</summary>
        private static readonly int TargetEdgeUv = Shader.PropertyToID("_TargetEdgeUv");

        /// <summary>リングの縮尺。1 で通常、0 で消えている。</summary>
        private static readonly int Phase = Shader.PropertyToID("_Phase");

        /// <summary>波紋のリングがいまどの UV 半径にいるか。</summary>
        private static readonly int RippleR = Shader.PropertyToID("_RippleR");

        /// <summary>波紋の濃さ。0 なら波紋なし。</summary>
        private static readonly int RippleA = Shader.PropertyToID("_RippleA");

        /// <summary>波紋が広がりきる UV 半径。四角の縁がここ。</summary>
        private const float RippleOuterUv = 0.5f;

        private SpriteRenderer _renderer;
        private MaterialPropertyBlock _block;
        private WorldCoordinateTransform _world;
        private Target _target;

        private Motion _motion = Motion.Idle;
        private float _elapsed;

        /// <summary>収縮が終わったら移る先。</summary>
        private Vector3 _destination;

        private void Awake()
        {
            CacheRenderer();
        }

        private void CacheRenderer()
        {
            if (_renderer == null) _renderer = GetComponent<SpriteRenderer>();

            if (_renderer == null) return;

            // 絵が割り当てられていなければ用意する。
            // 形をシェーダが描くマテリアル (NeonRing) のときは下敷きの四角でよく、
            // そうでなければ仮の円を置く。素材ができたら Prefab 側で差し替える。
            if (_renderer.sprite == null)
            {
                _renderer.sprite = DrawsItsOwnShape
                    ? PlaceholderSprites.Quad
                    : PlaceholderSprites.Circle;
            }
        }

        public void Initialize(Target target, WorldCoordinateTransform world)
        {
            _world = world;
            _target = target;
            CacheRenderer();

            transform.localPosition = ToWorld(target);
            ApplyDiameter(target);

            _motion = Motion.Idle;
            _elapsed = 0f;
            Draw(phase: 1f, rippleRadius: 0f, rippleAlpha: 0f);

            // 生成直後は落ち着いた状態。演出の途中で作り直されても当たるように戻す。
            target?.EndRelocation();
        }

        public void MoveTo(Target target)
        {
            _target = target;
            _destination = ToWorld(target);

            // 演出を切ったときは従来どおり瞬間移動する。
            if (_collapseSeconds <= 0f && _expandSeconds <= 0f)
            {
                transform.localPosition = _destination;
                ApplyDiameter(target);
                _motion = Motion.Idle;
                Draw(1f, 0f, 0f);
                target.EndRelocation();
                return;
            }

            // 膨らんでいる途中でまた当たることがある。そのときは現在の大きさから
            // 縮め直す。頭から縮めると一瞬大きくなって見えるため。
            _elapsed = _motion == Motion.Expanding
                ? Mathf.Max(0f, _collapseSeconds * (1f - CurrentPhase()))
                : 0f;

            _motion = Motion.Collapsing;
        }

        private void Update()
        {
            if (_motion == Motion.Idle) return;

            _elapsed += Time.deltaTime;

            if (_motion == Motion.Collapsing)
            {
                float t = Progress(_elapsed, _collapseSeconds);

                // 縮みは終盤ほど速い。吸い込まれるように見せる。
                float phase = 1f - t * t;

                // 波紋はリングがあった場所から外へ抜けていく。
                Draw(phase, Mathf.Lerp(EdgeUv(), RippleOuterUv, t), (1f - t) * _rippleStrength);

                if (t < 1f) return;

                // 消え切ったところで移す。軌跡は見せない。
                transform.localPosition = _destination;
                if (_target != null) ApplyDiameter(_target);

                _motion = Motion.Expanding;
                _elapsed = 0f;
                return;
            }

            // Expanding
            {
                float t = Progress(_elapsed, _expandSeconds);

                // 膨らみは終盤ほど遅い。止まりぎわが落ち着く。
                float phase = 1f - (1f - t) * (1f - t);

                // 波紋は中心から湧いて外へ広がる。
                Draw(phase, Mathf.Lerp(0f, RippleOuterUv, t), (1f - t) * _rippleStrength);

                if (t < 1f) return;

                _motion = Motion.Idle;
                Draw(1f, 0f, 0f);

                // ここでようやく移動先に現れ切った。当たり判定を戻す。
                // GameRuntime.MoveAfterHit が外したものの対。
                _target?.EndRelocation();
            }
        }

        /// <summary>0 秒の指定を 0 除算にせず「即完了」として扱う。</summary>
        private static float Progress(float elapsed, float duration) =>
            duration <= 0f ? 1f : Mathf.Clamp01(elapsed / duration);

        /// <summary>いまのリング縮尺。途中で当たり直されたときの継ぎ目に使う。</summary>
        private float CurrentPhase()
        {
            if (_motion == Motion.Expanding)
            {
                float t = Progress(_elapsed, _expandSeconds);
                return 1f - (1f - t) * (1f - t);
            }

            if (_motion == Motion.Collapsing)
            {
                float t = Progress(_elapsed, _collapseSeconds);
                return 1f - t * t;
            }

            return 1f;
        }

        /// <summary>
        /// 演出の値を的ごとに渡す。
        ///
        /// MaterialPropertyBlock なのでマテリアルは複製されない。Inspector で
        /// 詰めた値はそのまま効き、的 4 個が別々のタイミングで動ける。
        /// </summary>
        private void Draw(float phase, float rippleRadius, float rippleAlpha)
        {
            if (_renderer == null) return;

            _block ??= new MaterialPropertyBlock();
            _renderer.GetPropertyBlock(_block);
            _block.SetFloat(Phase, Mathf.Clamp01(phase));
            _block.SetFloat(RippleR, rippleRadius);
            _block.SetFloat(RippleA, Mathf.Max(0f, rippleAlpha));
            _renderer.SetPropertyBlock(_block);
        }

        /// <summary>
        /// マテリアルが「的の境界が UV 半径のどこか」を宣言しているか。
        /// 宣言していれば形はシェーダが描いている。
        /// </summary>
        private bool DrawsItsOwnShape =>
            _renderer != null
            && _renderer.sharedMaterial != null
            && _renderer.sharedMaterial.HasFloat(TargetEdgeUv);

        /// <summary>的の境界の UV 半径。宣言が無ければ絵の全幅を的とみなす。</summary>
        private float EdgeUv()
        {
            if (!DrawsItsOwnShape) return RippleOuterUv;

            float edgeUv = _renderer.sharedMaterial.GetFloat(TargetEdgeUv);
            return edgeUv > 0f ? edgeUv : RippleOuterUv;
        }

        /// <summary>
        /// Target.Size は直径。CollisionSolver は Radius (= Size / 2) で判定するので、
        /// 的の境界が直径に一致していれば見た目と当たり判定が一致する。
        ///
        /// スプライトの実寸 (bounds) を見てから倍率を出しているのは、
        /// 割り当てられた絵の pixelsPerUnit や余白がいくつでも合うようにするため。
        /// 「1 unit 角の絵が来る」と決め打つと、素材を差し替えた瞬間にずれる。
        ///
        /// さらに、形をシェーダが描く場合は絵の縁と的の境界が一致しない
        /// (外側にグローと波紋を置く余白があるため)。境界がどこかはマテリアルだけが
        /// 知っているので、ここで問い合わせる。
        /// </summary>
        private void ApplyDiameter(Target target)
        {
            if (_world == null) return;

            float desired = _world.ToWorldLength(target.Diameter);

            float spriteWidth = _renderer != null && _renderer.sprite != null
                ? _renderer.sprite.bounds.size.x
                : 0f;

            if (spriteWidth <= 0f)
            {
                Debug.LogWarning($"{name}: sprite has no width; cannot size {target}.");
                return;
            }

            // 絵の幅のうち、的の直径にあたる割合。
            float visibleFraction = EdgeUv() * 2f;

            transform.localScale = Vector3.one * (desired / (spriteWidth * visibleFraction));
        }

        private Vector3 ToWorld(Target target) => new(
            _world.ToWorldX(target.Coordinate.X),
            _world.ToWorldY(target.Coordinate.Y),
            0f);
    }
}
