using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.Win32;
using SolusManifestApp.Helpers;
using SolusManifestApp.Models;
using SolusManifestApp.Services;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;

namespace SolusManifestApp.Views.Dialogs
{
    public partial class ProfileViewModel : ObservableObject
    {
        public string Id { get; }
        public string Name { get; set; }
        public int GameCount { get; }

        private bool _isActive;
        public bool IsActive
        {
            get => _isActive;
            set => SetProperty(ref _isActive, value);
        }

        public ProfileViewModel(GreenLumaProfile profile, bool isActive)
        {
            Id = profile.Id;
            Name = profile.Name;
            GameCount = profile.Games.Count;
            IsActive = isActive;
        }
    }

    public partial class ProfileManagerDialog : Window
    {
        private readonly ProfileService _profileService;
        private readonly SteamApiService? _steamApiService;
        private List<ProfileViewModel> _viewModels = new();
        public bool ProfilesChanged { get; private set; }

        public ProfileManagerDialog(ProfileService profileService, SteamApiService? steamApiService = null)
        {
            InitializeComponent();
            _profileService = profileService;
            _steamApiService = steamApiService;
            LoadProfiles();
        }

        private void LoadProfiles()
        {
            var data = _profileService.LoadProfiles();
            _viewModels = data.Profiles
                .Select(p => new ProfileViewModel(p, p.Id == data.ActiveProfileId))
                .ToList();

            ProfileListBox.ItemsSource = _viewModels;

            var activeProfile = _profileService.GetActiveProfile();
            ActiveProfileNameRun.Text = activeProfile?.Name ?? "None";

            UpdateAppliedStatus();
        }

        private void UpdateAppliedStatus()
        {
            var activeProfile = _profileService.GetActiveProfile();
            if (activeProfile == null)
            {
                AppliedStatusText.Text = "No active profile";
                AppliedStatusText.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("#FF6B6B"));
                return;
            }

            var isApplied = _profileService.IsProfileApplied(activeProfile.Id);
            if (isApplied)
            {
                AppliedStatusText.Text = "Profile is applied to AppList";
                AppliedStatusText.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("#4CAF50"));
            }
            else
            {
                AppliedStatusText.Text = "Profile not applied - click Apply to update AppList";
                AppliedStatusText.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("#FFA500"));
            }
        }

        private void CreateProfile_Click(object sender, RoutedEventArgs e)
        {
            var dialog = new InputDialog("Create Profile", "Enter profile name:");
            if (dialog.ShowDialog() == true && !string.IsNullOrWhiteSpace(dialog.InputText))
            {
                _profileService.CreateProfile(dialog.InputText.Trim());
                ProfilesChanged = true;
                LoadProfiles();
            }
        }

        private void RenameProfile_Click(object sender, RoutedEventArgs e)
        {
            if (sender is not FrameworkElement element || element.Tag is not string profileId)
                return;

            var profile = _profileService.GetProfileById(profileId);
            if (profile == null)
                return;

            var dialog = new InputDialog("Rename Profile", "Enter new name:", profile.Name);
            if (dialog.ShowDialog() == true && !string.IsNullOrWhiteSpace(dialog.InputText))
            {
                _profileService.RenameProfile(profileId, dialog.InputText.Trim());
                ProfilesChanged = true;
                LoadProfiles();
            }
        }

        private async void DeleteProfile_Click(object sender, RoutedEventArgs e)
        {
            if (sender is not FrameworkElement element || element.Tag is not string profileId)
                return;

            var profile = _profileService.GetProfileById(profileId);
            if (profile == null)
                return;

            var result = MessageBoxHelper.Show(
                $"Are you sure you want to delete the profile '{profile.Name}'?\n\nGames that don't exist in any other profile will be UNINSTALLED (removed from AppList, manifest files deleted).",
                "Delete Profile",
                MessageBoxButton.YesNo,
                MessageBoxImage.Warning);

            if (result == MessageBoxResult.Yes)
            {
                await _profileService.DeleteProfileAsync(profileId);
                ProfilesChanged = true;
                LoadProfiles();
            }
        }

        private void ViewGames_Click(object sender, RoutedEventArgs e)
        {
            if (sender is not FrameworkElement element || element.Tag is not string profileId)
                return;

            var profile = _profileService.GetProfileById(profileId);
            if (profile == null)
                return;

            var dialog = new ProfileGamesDialog(profile, _profileService);
            dialog.Owner = this;
            if (dialog.ShowDialog() == true && dialog.GamesChanged)
            {
                ProfilesChanged = true;
                LoadProfiles();
            }
        }

        private void ExportProfile_Click(object sender, RoutedEventArgs e)
        {
            if (sender is not FrameworkElement element || element.Tag is not string profileId)
                return;

            var profile = _profileService.GetProfileById(profileId);
            if (profile == null)
                return;

            var saveDialog = new SaveFileDialog
            {
                Filter = "Profile Package (*.zip)|*.zip",
                FileName = $"{profile.Name}_profile.zip",
                Title = "Export Profile"
            };

            if (saveDialog.ShowDialog() == true)
            {
                var (success, message, manifestCount) = _profileService.ExportProfileAsZip(profileId, saveDialog.FileName);

                if (success)
                {
                    MessageBoxHelper.Show(
                        $"Profile '{profile.Name}' exported successfully!\n\n{message}\n\nNote: Only games installed with profile tracking will be included in the export.",
                        "Export Successful",
                        MessageBoxButton.OK,
                        MessageBoxImage.Information);
                }
                else
                {
                    MessageBoxHelper.Show(
                        message,
                        "Export Failed",
                        MessageBoxButton.OK,
                        MessageBoxImage.Error);
                }
            }
        }

        private void ImportProfile_Click(object sender, RoutedEventArgs e)
        {
            var openDialog = new OpenFileDialog
            {
                Filter = "Profile Package (*.zip)|*.zip|JSON files (*.json)|*.json|All files (*.*)|*.*",
                Title = "Import Profile"
            };

            if (openDialog.ShowDialog() == true)
            {
                var (success, message, profile) = _profileService.ImportProfile(openDialog.FileName);

                if (success)
                {
                    MessageBoxHelper.Show(
                        $"{message}\n\nRestart Steam for changes to take effect.",
                        "Import Successful",
                        MessageBoxButton.OK,
                        MessageBoxImage.Information);
                    ProfilesChanged = true;
                    LoadProfiles();
                }
                else
                {
                    MessageBoxHelper.Show(
                        message,
                        "Import Failed",
                        MessageBoxButton.OK,
                        MessageBoxImage.Error);
                }
            }
        }

        private async void ImportAppList_Click(object sender, RoutedEventArgs e)
        {
            var result = MessageBoxHelper.Show(
                "This will create a new profile from your current AppList contents.\n\n" +
                "Note: Games imported this way will NOT have depot tracking information. " +
                "Only games installed after this feature was added will have full depot data.\n\n" +
                "Continue?",
                "Import Current AppList",
                MessageBoxButton.YesNo,
                MessageBoxImage.Information);

            if (result != MessageBoxResult.Yes)
                return;

            var dialog = new InputDialog("Import AppList", "Enter name for the new profile:", "Imported Profile");
            if (dialog.ShowDialog() != true || string.IsNullOrWhiteSpace(dialog.InputText))
                return;

            var profile = _profileService.ImportCurrentAppList(dialog.InputText.Trim());

            if (profile != null)
            {
                if (_steamApiService != null)
                {
                    var appList = await _steamApiService.GetAppListAsync();
                    if (appList != null)
                    {
                        foreach (var game in profile.Games)
                        {
                            var gameName = _steamApiService.GetGameName(game.AppId, appList);
                            if (!string.IsNullOrEmpty(gameName) && gameName != $"App {game.AppId}")
                            {
                                game.Name = gameName;
                            }
                        }
                        _profileService.SaveProfiles();
                    }
                }

                MessageBoxHelper.Show(
                    $"Imported {profile.Games.Count} apps to profile '{profile.Name}'.",
                    "Import Successful",
                    MessageBoxButton.OK,
                    MessageBoxImage.Information);
                ProfilesChanged = true;
                LoadProfiles();
            }
            else
            {
                MessageBoxHelper.Show(
                    "No apps found in AppList or failed to import.",
                    "Import Failed",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
            }
        }

        private void ProfileItem_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2 && sender is FrameworkElement element && element.DataContext is ProfileViewModel vm)
            {
                foreach (var profile in _viewModels)
                {
                    profile.IsActive = profile.Id == vm.Id;
                }

                _profileService.SetActiveProfile(vm.Id);
                ProfilesChanged = true;

                var activeProfile = _profileService.GetActiveProfile();
                ActiveProfileNameRun.Text = activeProfile?.Name ?? "None";
                UpdateAppliedStatus();
            }
        }

        private void ApplyProfile_Click(object sender, RoutedEventArgs e)
        {
            var activeProfile = _profileService.GetActiveProfile();
            if (activeProfile == null)
            {
                MessageBoxHelper.Show(
                    "No active profile selected.",
                    "Apply Profile",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
                return;
            }

            var totalEntries = activeProfile.Games.Count + activeProfile.Games.Sum(g => g.Depots.Count);

            var confirmResult = MessageBoxHelper.Show(
                $"This will REPLACE the entire AppList folder with {totalEntries} entries from profile '{activeProfile.Name}'.\n\n" +
                "All existing AppList entries will be removed.\n\n" +
                "Continue?",
                "Apply Profile",
                MessageBoxButton.YesNo,
                MessageBoxImage.Question);

            if (confirmResult != MessageBoxResult.Yes)
                return;

            var (success, message) = _profileService.ApplyProfile(activeProfile.Id);

            if (success)
            {
                MessageBoxHelper.Show(
                    message + "\n\nRestart Steam for changes to take effect.",
                    "Profile Applied",
                    MessageBoxButton.OK,
                    MessageBoxImage.Information);
                UpdateAppliedStatus();
            }
            else
            {
                MessageBoxHelper.Show(
                    message,
                    "Apply Failed",
                    MessageBoxButton.OK,
                    MessageBoxImage.Error);
            }
        }

        private void Close_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = ProfilesChanged;
            Close();
        }

        private void CloseButton_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = ProfilesChanged;
            Close();
        }

        private void TitleBar_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 1)
            {
                DragMove();
            }
        }
    }
}
