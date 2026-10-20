using Struckout.Launcher.Core.Launch;
using Struckout.Launcher.Core.Modules;

namespace Struckout.Launcher.Core.Tests;

public sealed class CameraTests : IDisposable
{
    private const string Cameras = """
        {
          "description": ["kept as it is"],
          "package": "com.taichi765.struckoutCameraApp.highspeed",
          "cameras": [
            {
              "serial": "166810122536",
              "name": "left",
              "output": "network",
              "focal_ratio": 0.97,
              "pose": [0.11, 0.12, 1.23, 136.962, -0.453, -32.331]
            },
            {
              "serial": "166810131208",
              "device_id": "3d7cc4fb2b85ac4f",
              "name": "right",
              "output": "network",
              "focal_ratio": 0.97,
              "pose": [0.11, 1.06, 1.23, 135.14, -1.542, -148.297],
              "area": [0, 0, 0.5, 1]
            }
          ]
        }
        """;

    private readonly string path = Path.Combine(Path.GetTempPath(), $"cameras-{Guid.NewGuid()}.json");

    public CameraTests() => File.WriteAllText(path, "﻿" + Cameras);

    public void Dispose() => File.Delete(path);

    [Fact]
    public void ReadsCamerasInOrder()
    {
        var file = CamerasFile.Load(path);

        Assert.Equal("com.taichi765.struckoutCameraApp.highspeed", file.Package);
        Assert.Equal(["left", "right"], file.Cameras.Select(c => c.Name));
        Assert.Null(file.Cameras[0].DeviceId);
        Assert.Equal([0, 0, 0.5, 1], file.Cameras[1].Area);
    }

    [Fact]
    public void RecordsDeviceIdAndKeepsTheRest()
    {
        var file = CamerasFile.Load(path);

        Assert.True(file.SetDeviceId("166810122536", "94674f368ce5be0e"));
        Assert.False(file.SetDeviceId("166810122536", "94674f368ce5be0e"));

        var reloaded = CamerasFile.Load(path);
        Assert.Equal("94674f368ce5be0e", reloaded.Cameras[0].DeviceId);
        Assert.Equal("94674f368ce5be0e", file.Cameras[0].DeviceId);
        Assert.Contains("kept as it is", File.ReadAllText(path), StringComparison.Ordinal);
    }

    [Fact]
    public void SendsTheSettingsTheAppExpects()
    {
        var right = CamerasFile.Load(path).Cameras[1];

        Assert.Equal(
            ["--es", "output", "network", "--es", "focal_ratio", "0.97",
                "--es", "pose", "0.11,1.06,1.23,135.14,-1.542,-148.297", "--es", "area", "0,0,0.5,1"],
            CameraRunner.ConfigExtras(right));
    }

    [Fact]
    public void ListsOnlyAuthorizedDevices()
    {
        const string output = "List of devices attached\r\n166810122536\tdevice\r\n166810131208\tunauthorized\r\nemulator-5554\toffline\r\n\r\n";

        Assert.Equal(["166810122536"], Adb.ParseDevices(output));
    }
}
