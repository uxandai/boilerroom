using Microsoft.Win32;
using System;
using System.Diagnostics;
using System.IO;

namespace SolusManifestApp.Helpers
{
    public static class ProtocolRegistrationHelper
    {
        private const string ProtocolName = "solusapp";
        private const string RegistryPath = @"Software\Classes\" + ProtocolName;

        public static bool IsProtocolRegistered()
        {
            try
            {
                using var key = Registry.CurrentUser.OpenSubKey(RegistryPath);
                return key != null;
            }
            catch
            {
                return false;
            }
        }

        public static bool RegisterProtocol()
        {
            try
            {
                var exePath = Process.GetCurrentProcess().MainModule?.FileName;
                if (string.IsNullOrEmpty(exePath))
                    return false;

                // Check if already registered with correct path
                var commandPath = $@"{RegistryPath}\shell\open\command";
                using (var commandKey = Registry.CurrentUser.OpenSubKey(commandPath))
                {
                    if (commandKey != null)
                    {
                        var currentCommand = commandKey.GetValue("")?.ToString() ?? "";
                        var expectedCommand = $"\"{exePath}\" \"%1\"";

                        // If path matches, no need to re-register
                        if (currentCommand.Equals(expectedCommand, StringComparison.OrdinalIgnoreCase))
                        {
                            return true;
                        }

                        // Path is different, delete old registration
                        UnregisterProtocol();
                    }
                }

                // Register with new path
                using var key = Registry.CurrentUser.CreateSubKey(RegistryPath);
                key.SetValue("", $"URL:{ProtocolName} Protocol");
                key.SetValue("URL Protocol", "");

                using var defaultIcon = key.CreateSubKey("DefaultIcon");
                defaultIcon.SetValue("", $"\"{exePath}\",0");

                using var command = key.CreateSubKey(@"shell\open\command");
                command.SetValue("", $"\"{exePath}\" \"%1\"");

                return true;
            }
            catch
            {
                return false;
            }
        }

        public static void UnregisterProtocol()
        {
            try
            {
                Registry.CurrentUser.DeleteSubKeyTree(RegistryPath, false);
            }
            catch
            {
                // Ignore errors during unregistration
            }
        }

        public static string? ParseProtocolUrl(string url)
        {
            if (string.IsNullOrEmpty(url))
                return null;

            // Handle both formats: solusapp://download/install/400 and "solusapp://download/install/400"
            var cleanUrl = url.Trim('"', ' ');

            if (!cleanUrl.StartsWith($"{ProtocolName}://", StringComparison.OrdinalIgnoreCase))
                return null;

            // Remove the protocol prefix
            return cleanUrl.Substring($"{ProtocolName}://".Length);
        }
    }
}
