using YamlDotNet.Core;
using YamlDotNet.Serialization;
using YamlDotNet.Serialization.NamingConventions;

namespace Struckout.Launcher.Core.Spec;

/// <summary>The TCP ports the modules listen on.</summary>
/// <remarks>
/// They're read from <c>api/spec/servers.yaml</c>, which is embedded at build time, so that the
/// launcher never disagrees with the modules about a port (the Rust crates read the same file in build.rs).
/// </remarks>
public sealed record ServerPorts(
    int GameMasterGrpc,
    int BallTrackerProjector,
    int BallTrackerCameraControl,
    int BallTrackerCameraData,
    int BallTrackerCameraVideo)
{
    private static readonly Lazy<ServerPorts> FromSpec = new(LoadEmbedded);

    /// <summary>The ports in the embedded <c>api/spec/servers.yaml</c>.</summary>
    public static ServerPorts Spec => FromSpec.Value;

    /// <exception cref="InvalidDataException">The YAML doesn't define every port.</exception>
    internal static ServerPorts Parse(string yaml)
    {
        SpecFile spec;
        try
        {
            spec = new DeserializerBuilder()
                .WithNamingConvention(CamelCaseNamingConvention.Instance)
                .Build()
                .Deserialize<SpecFile>(yaml);
        }
        catch (YamlException e)
        {
            throw new InvalidDataException($"servers.yaml is malformed: {e.Message}", e);
        }

        return new ServerPorts(
            Port(spec, "game-master", "grpc"),
            Port(spec, "ball-tracker", "projector"),
            Port(spec, "ball-tracker", "camera-ctrl"),
            Port(spec, "ball-tracker", "camera-data"),
            Port(spec, "ball-tracker", "camera-video"));
    }

    private static int Port(SpecFile spec, string server, string endpoint)
    {
        if (spec.Servers.TryGetValue(server, out var endpoints) && endpoints.TryGetValue(endpoint, out var definition))
        {
            return definition.Port;
        }

        throw new InvalidDataException($"servers.yaml has no servers.{server}.{endpoint}");
    }

    private static ServerPorts LoadEmbedded()
    {
        using var stream = typeof(ServerPorts).Assembly.GetManifestResourceStream("servers.yaml")
            // guaranteed by the EmbeddedResource in Struckout.Launcher.Core.csproj
            ?? throw new InvalidOperationException("servers.yaml should be embedded");
        using var reader = new StreamReader(stream);
        return Parse(reader.ReadToEnd());
    }

    private sealed class SpecFile
    {
        public Dictionary<string, Dictionary<string, Endpoint>> Servers { get; set; } = [];
    }

    private sealed class Endpoint
    {
        public string Type { get; set; } = "";

        public int Port { get; set; }
    }
}
