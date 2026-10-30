using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Launch;

/// <summary>The <c>launcher.toml</c> the installer writes, with a comment on every key.</summary>
public static class LauncherConfigTemplate
{
    /// <summary>A config that starts <paramref name="modules"/>.</summary>
    public static string Render(IEnumerable<ModuleId> modules)
    {
        var list = string.Join(", ", modules.Order().Select(m => $"\"{m.Slug()}\""));
        return $$"""
            # struckout のランチャーの設定。書き換えたら、ランチャーの「設定を読み直す」を押す。

            # この PC で起動するモジュール。書いた順ではなく、依存の順に起動する
            # (game-master と ball-tracker が先、camera と projector、touchpanel が後)。
            # 使えるもの: "ball-tracker", "camera", "game-master", "projector", "touchpanel"
            # camera は adb reverse でこの PC の ball-tracker につなぐので、ball-tracker と同じ PC にする。
            modules = [{{list}}]

            # この筐体の番号。projector が game-master に名乗る。touchpanel は自分の config.toml を使う。
            machine-id = 1

            # ほかの PC で動かすモジュールのつなぎ先 (PC の名前か IP)。
            # modules に入っているモジュールには、ここの値によらず、この PC (localhost) でつなぐ。
            [hosts]
            ball-tracker = "localhost"
            game-master = "localhost"

            [ball-tracker]
            # 着弾をどこに送るか: "network" (projector) か "csv" (projector なしで試すとき)
            output = "network"
            # output = "csv" のときの書き出し先。この設定ファイルからの相対パスでもよい
            # csv-path = "collisions.csv"
            # ball_tracker にそのまま渡す引数。例: ["--time-trim-ms", "3"]
            extra-args = []

            [touchpanel]
            # 受付画面のディスプレイ。省略するとメインの画面に全画面表示する。
            # winit の列挙順で 1 から。Unity (projector.monitor) や Windows の番号と一致するとは限らない。
            # monitor = 1

            [projector]
            # 的を映す画面の番号 (1 から)。PC の画面が 1 なら、プロジェクターはふつう 2
            monitor = 2
            fullscreen = true
            # 着弾位置の係数のファイル (projector の F2 の調整画面で保存したもの)。空なら projector の既定
            collision-transform = ""

            """;
    }

    /// <summary>Writes the template to <paramref name="path"/> unless the user already has a config.</summary>
    /// <returns>Whether the file was written.</returns>
    public static bool WriteIfMissing(string path, IEnumerable<ModuleId> modules)
    {
        if (File.Exists(path))
        {
            return false;
        }

        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
        File.WriteAllText(path, Render(modules));
        return true;
    }
}
