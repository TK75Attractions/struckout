namespace Struckout.TestCli;

/// <summary>
/// 受信ループや auto 送信はバックグラウンドで動くので、
/// プロンプトと出力が混ざらないように書き込みを直列化する。
/// </summary>
public static class ConsoleLog
{
    private static readonly Lock Gate = new();

    public static void Write(string source, string message)
    {
        lock (Gate)
        {
            Console.WriteLine($"[{DateTime.Now:HH:mm:ss.fff}] [{source}] {message}");
        }
    }

    public static void Plain(string message)
    {
        lock (Gate)
        {
            Console.WriteLine(message);
        }
    }
}
