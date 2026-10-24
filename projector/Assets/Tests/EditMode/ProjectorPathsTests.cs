using System;
using System.IO;
using NUnit.Framework;
using Struckout.Infrastructure;
using UnityEngine;

namespace Struckout.Tests.Tests.EditMode
{
    public class ProjectorPathsTests
    {
        private static string Home => Path.Combine(Path.GetTempPath(), "struckout-home");

        [TestCase(RuntimePlatform.LinuxEditor)]
        [TestCase(RuntimePlatform.LinuxPlayer)]
        public void LinuxUsesXdgConfigHomeEvenIfItDoesNotExist(RuntimePlatform platform)
        {
            var config = Path.Combine(Home, "xdg-config");
            var directory = ProjectorPaths.ResolveConfigDirectory(
                platform, name => name == "XDG_CONFIG_HOME" ? config : null, _ => null);

            Assert.That(directory, Is.EqualTo(Path.Combine(config, "struckout", "projector")));
        }

        [TestCase(null)]
        [TestCase("")]
        [TestCase("relative/config")]
        public void LinuxFallsBackToHomeForMissingOrRelativeXdgConfigHome(string xdgConfigHome)
        {
            var directory = ProjectorPaths.ResolveConfigDirectory(
                RuntimePlatform.LinuxPlayer, _ => xdgConfigHome,
                folder => folder == Environment.SpecialFolder.UserProfile ? Home : null);

            Assert.That(directory, Is.EqualTo(Path.Combine(Home, ".config", "struckout", "projector")));
        }

        [TestCase(RuntimePlatform.WindowsEditor)]
        [TestCase(RuntimePlatform.WindowsPlayer)]
        public void WindowsUsesRoamingApplicationData(RuntimePlatform platform)
        {
            var roaming = Path.Combine(Home, "AppData", "Roaming");
            var directory = ProjectorPaths.ResolveConfigDirectory(
                platform, _ => throw new AssertionException("Windows must ignore XDG variables"),
                folder => folder == Environment.SpecialFolder.ApplicationData ? roaming : null);

            Assert.That(directory, Is.EqualTo(Path.Combine(roaming, "struckout", "projector")));
        }

        [TestCase(RuntimePlatform.OSXEditor)]
        [TestCase(RuntimePlatform.OSXPlayer)]
        public void MacUsesApplicationSupport(RuntimePlatform platform)
        {
            var directory = ProjectorPaths.ResolveConfigDirectory(
                platform, _ => throw new AssertionException("macOS must ignore XDG variables"),
                folder => folder == Environment.SpecialFolder.UserProfile ? Home : null);

            Assert.That(directory,
                Is.EqualTo(Path.Combine(Home, "Library", "Application Support", "struckout", "projector")));
        }

        [TestCase(RuntimePlatform.WindowsPlayer)]
        [TestCase(RuntimePlatform.OSXPlayer)]
        [TestCase(RuntimePlatform.LinuxPlayer)]
        public void MissingUserDirectoryFailsInsteadOfUsingWorkingDirectory(RuntimePlatform platform)
        {
            Assert.Throws<InvalidOperationException>(() =>
                ProjectorPaths.ResolveConfigDirectory(platform, _ => null, _ => ""));
        }

        [Test]
        public void UnsupportedPlatformFailsExplicitly()
        {
            Assert.Throws<PlatformNotSupportedException>(() =>
                ProjectorPaths.ResolveConfigDirectory(RuntimePlatform.WebGLPlayer, _ => null, _ => Home));
        }
    }
}
