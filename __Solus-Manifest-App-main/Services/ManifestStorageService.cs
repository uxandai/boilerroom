using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace SolusManifestApp.Services
{
    public class InstalledManifestInfo
    {
        public string AppId { get; set; } = "";
        public string GameName { get; set; } = "";
        public ulong ManifestId { get; set; }
        public DateTime InstalledDate { get; set; }
        public string InstallPath { get; set; } = "";
        public List<uint> DepotIds { get; set; } = new();
    }

    public class ManifestStorageService
    {
        private readonly LoggerService _logger;
        private readonly string _manifestFolder;
        private readonly string _indexFilePath;
        private Dictionary<string, InstalledManifestInfo> _installedManifests = new();

        public string ManifestFolder => _manifestFolder;

        public ManifestStorageService(LoggerService logger)
        {
            _logger = logger;
            _manifestFolder = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                "SolusManifestApp",
                "Manifests"
            );
            _indexFilePath = Path.Combine(_manifestFolder, "manifest_index.json");

            EnsureDirectoryExists();
            LoadIndex();
        }

        private void EnsureDirectoryExists()
        {
            if (!Directory.Exists(_manifestFolder))
            {
                Directory.CreateDirectory(_manifestFolder);
                _logger.Debug($"Created manifest storage folder: {_manifestFolder}");
            }
        }

        private void LoadIndex()
        {
            try
            {
                if (File.Exists(_indexFilePath))
                {
                    var json = File.ReadAllText(_indexFilePath);
                    _installedManifests = JsonSerializer.Deserialize<Dictionary<string, InstalledManifestInfo>>(json) ?? new();
                    _logger.Debug($"Loaded {_installedManifests.Count} manifest entries from index");
                }
            }
            catch (Exception ex)
            {
                _logger.Error($"Failed to load manifest index: {ex.Message}");
                _installedManifests = new();
            }
        }

        private void SaveIndex()
        {
            try
            {
                var json = JsonSerializer.Serialize(_installedManifests, new JsonSerializerOptions { WriteIndented = true });
                File.WriteAllText(_indexFilePath, json);
                _logger.Debug("Saved manifest index");
            }
            catch (Exception ex)
            {
                _logger.Error($"Failed to save manifest index: {ex.Message}");
            }
        }

        public void StoreManifest(string appId, string gameName, ulong manifestId, string installPath, List<uint>? depotIds = null)
        {
            var info = new InstalledManifestInfo
            {
                AppId = appId,
                GameName = gameName,
                ManifestId = manifestId,
                InstalledDate = DateTime.Now,
                InstallPath = installPath,
                DepotIds = depotIds ?? new()
            };

            _installedManifests[appId] = info;
            SaveIndex();

            _logger.Info($"Stored manifest info for {gameName} (AppId: {appId}, ManifestId: {manifestId})");
        }

        public void StoreManifestFile(string appId, uint depotId, ulong manifestId, byte[] manifestData)
        {
            try
            {
                var fileName = $"{appId}_{depotId}_{manifestId}.manifest";
                var filePath = Path.Combine(_manifestFolder, fileName);
                File.WriteAllBytes(filePath, manifestData);
                _logger.Debug($"Stored manifest file: {fileName}");
            }
            catch (Exception ex)
            {
                _logger.Error($"Failed to store manifest file: {ex.Message}");
            }
        }

        public byte[]? GetManifestFile(string appId, uint depotId, ulong manifestId)
        {
            try
            {
                var fileName = $"{appId}_{depotId}_{manifestId}.manifest";
                var filePath = Path.Combine(_manifestFolder, fileName);
                if (File.Exists(filePath))
                {
                    return File.ReadAllBytes(filePath);
                }
            }
            catch (Exception ex)
            {
                _logger.Error($"Failed to read manifest file: {ex.Message}");
            }
            return null;
        }

        public InstalledManifestInfo? GetInstalledManifest(string appId)
        {
            return _installedManifests.TryGetValue(appId, out var info) ? info : null;
        }

        public bool IsInstalled(string appId)
        {
            return _installedManifests.ContainsKey(appId);
        }

        public ulong? GetInstalledManifestId(string appId)
        {
            return _installedManifests.TryGetValue(appId, out var info) ? info.ManifestId : null;
        }

        public bool HasUpdate(string appId, ulong latestManifestId)
        {
            var installed = GetInstalledManifestId(appId);
            if (installed == null)
                return false;

            return installed.Value != latestManifestId;
        }

        public void RemoveManifest(string appId)
        {
            if (_installedManifests.Remove(appId))
            {
                SaveIndex();
                _logger.Debug($"Removed manifest info for AppId: {appId}");
            }
        }

        public IEnumerable<InstalledManifestInfo> GetAllInstalledManifests()
        {
            return _installedManifests.Values;
        }

        public void ClearAll()
        {
            _installedManifests.Clear();
            SaveIndex();

            foreach (var file in Directory.GetFiles(_manifestFolder, "*.manifest"))
            {
                try
                {
                    File.Delete(file);
                }
                catch { }
            }

            _logger.Info("Cleared all stored manifests");
        }
    }
}
