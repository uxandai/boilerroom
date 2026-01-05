using CommunityToolkit.Mvvm.ComponentModel;
using SolusManifestApp.Models;
using System.Collections.Generic;
using System.Linq;
using System.Windows;

namespace SolusManifestApp.Views.Dialogs
{
    public partial class ProfileSelectionViewModel : ObservableObject
    {
        public string Id { get; }
        public string Name { get; }
        public int GameCount { get; }
        public bool IsActive { get; }

        private bool _isSelected;
        public bool IsSelected
        {
            get => _isSelected;
            set => SetProperty(ref _isSelected, value);
        }

        public ProfileSelectionViewModel(GreenLumaProfile profile, bool isActive, bool isSelected)
        {
            Id = profile.Id;
            Name = profile.Name;
            GameCount = profile.Games.Count;
            IsActive = isActive;
            _isSelected = isSelected;
        }
    }

    public partial class ProfileSelectionDialog : Window
    {
        private readonly List<ProfileSelectionViewModel> _viewModels;

        public List<string> SelectedProfileIds { get; private set; } = new();

        public ProfileSelectionDialog(List<GreenLumaProfile> profiles, string activeProfileId)
        {
            InitializeComponent();

            _viewModels = profiles
                .OrderByDescending(p => p.Id == activeProfileId)
                .ThenBy(p => p.Name)
                .Select(p => new ProfileSelectionViewModel(p, p.Id == activeProfileId, p.Id == activeProfileId))
                .ToList();

            ProfileListBox.ItemsSource = _viewModels;
        }

        private void Continue_Click(object sender, RoutedEventArgs e)
        {
            SelectedProfileIds = _viewModels
                .Where(vm => vm.IsSelected)
                .Select(vm => vm.Id)
                .ToList();

            if (SelectedProfileIds.Count == 0)
            {
                Helpers.MessageBoxHelper.Show(
                    "Please select at least one profile to continue.",
                    "No Profile Selected",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
                return;
            }

            DialogResult = true;
            Close();
        }

        private void Cancel_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = false;
            Close();
        }

        private void TitleBar_MouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if (e.ClickCount == 1)
            {
                DragMove();
            }
        }

        private void CloseButton_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = false;
            Close();
        }
    }
}
