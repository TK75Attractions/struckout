using Struckout.Launcher.Core.Install;
using Struckout.Launcher.Core.Platform;

namespace Struckout.Launcher.Core.Tests;

public class TouchpanelConfigTemplateTests
{
    [Fact]
    public void CreatesConfigAtTouchpanelPathAndPreservesEdits()
    {
        var directory = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString());
        try
        {
            var paths = new AppPaths(Path.Combine(directory, "struckout", "launcher"), Path.Combine(directory, "data"));

            Assert.Equal(Path.Combine(directory, "struckout", "touchpanel", "config.toml"), paths.TouchpanelConfigFile);
            Assert.True(TouchpanelConfigTemplate.WriteIfMissing(paths.TouchpanelConfigFile));
            Assert.Contains("address = \"127.0.0.1\"", File.ReadAllText(paths.TouchpanelConfigFile), StringComparison.Ordinal);
            Assert.Contains("machine-id = 1", File.ReadAllText(paths.TouchpanelConfigFile), StringComparison.Ordinal);
            Assert.Contains("simple-mode = false", File.ReadAllText(paths.TouchpanelConfigFile), StringComparison.Ordinal);

            File.WriteAllText(paths.TouchpanelConfigFile, "address = \"pc2.local\"\nmachine-id = 2\n");
            Assert.False(TouchpanelConfigTemplate.WriteIfMissing(paths.TouchpanelConfigFile));
            Assert.Equal("address = \"pc2.local\"\nmachine-id = 2\n", File.ReadAllText(paths.TouchpanelConfigFile));
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }
    }
}
