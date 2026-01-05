using SolusManifestApp.Helpers;
using SolusManifestApp.Models;
using SolusManifestApp.Services;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Input;

namespace SolusManifestApp.Views.Dialogs
{
    public class ProfileGameViewModel
    {
        public string AppId { get; set; } = string.Empty;
        public string Name { get; set; } = string.Empty;
        public int DepotCount { get; set; }
    }

    public partial class ProfileGamesDialog : Window
    {
        private readonly GreenLumaProfile _profile;
        private readonly ProfileService _profileService;
        private List<ProfileGameViewModel> _allGames = new();
        public bool GamesChanged { get; private set; }

        public ProfileGamesDialog(GreenLumaProfile profile, ProfileService profileService)
        {
            InitializeComponent();
            _profile = profile;
            _profileService = profileService;

            TitleText.Text = $"Games in '{profile.Name}'";
            LoadGames();
        }

        private void LoadGames()
        {
            var freshProfile = _profileService.GetProfileById(_profile.Id);
            if (freshProfile == null)
                return;

            _allGames = freshProfile.Games
                .Select(g => new ProfileGameViewModel
                {
                    AppId = g.AppId,
                    Name = g.Name,
                    DepotCount = g.Depots.Count
                })
                .OrderBy(g => g.Name)
                .ToList();

            ProfileInfoText.Text = $"{_allGames.Count} game(s) in this profile";
            ApplyFilter();
        }

        private void ApplyFilter()
        {
            var query = SearchBox.Text.ToLower().Trim();
            var filtered = string.IsNullOrEmpty(query)
                ? _allGames
                : _allGames.Where(g =>
                    g.Name.ToLower().Contains(query) ||
                    g.AppId.ToLower().Contains(query)).ToList();

            GamesListBox.ItemsSource = filtered;
        }

        private void SearchBox_TextChanged(object sender, System.Windows.Controls.TextChangedEventArgs e)
        {
            ApplyFilter();
        }

        private async void RemoveGame_Click(object sender, RoutedEventArgs e)
        {
            if (sender is not FrameworkElement element || element.Tag is not string appId)
                return;

            var game = _allGames.FirstOrDefault(g => g.AppId == appId);
            if (game == null)
                return;

            var result = MessageBoxHelper.Show(
                $"Remove '{game.Name}' from this profile?\n\nIf this game doesn't exist in any other profile, it will be UNINSTALLED (removed from AppList, manifest files deleted).",
                "Remove Game",
                MessageBoxButton.YesNo,
                MessageBoxImage.Warning);

            if (result == MessageBoxResult.Yes)
            {
                await _profileService.RemoveGameFromProfileAsync(_profile.Id, appId);
                GamesChanged = true;
                LoadGames();
            }
        }

        private void Close_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = GamesChanged;
            Close();
        }

        private void CloseButton_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = GamesChanged;
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
