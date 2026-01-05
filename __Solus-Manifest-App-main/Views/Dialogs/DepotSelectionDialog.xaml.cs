using SolusManifestApp.Helpers;
using CommunityToolkit.Mvvm.ComponentModel;
using SolusManifestApp.Services;
using System.Collections.Generic;
using System.Linq;
using System.Windows;

namespace SolusManifestApp.Views.Dialogs
{
    public partial class DepotSelectionViewModel : ObservableObject
    {
        private readonly DepotInfo _depotInfo;

        public string DepotId => _depotInfo.DepotId;
        public string Name => _depotInfo.Name;
        public string Language => _depotInfo.Language ?? "Unknown";
        public string SizeFormatted => FormatSize(_depotInfo.Size);
        public bool IsTokenBased => _depotInfo.IsTokenBased;
        public bool IsMainAppId => _depotInfo.IsMainAppId;

        private bool _isSelected;
        public bool IsSelected
        {
            get => _isSelected;
            set => SetProperty(ref _isSelected, value);
        }

        public DepotSelectionViewModel(DepotInfo depotInfo)
        {
            _depotInfo = depotInfo;
            _isSelected = depotInfo.IsSelected;
        }

        public DepotInfo GetDepotInfo()
        {
            _depotInfo.IsSelected = IsSelected;
            return _depotInfo;
        }

        private string FormatSize(long bytes)
        {
            if (bytes >= 1_073_741_824)
                return $"{bytes / 1_073_741_824.0:F2} GB";
            if (bytes >= 1_048_576)
                return $"{bytes / 1_048_576.0:F2} MB";
            if (bytes >= 1024)
                return $"{bytes / 1024.0:F2} KB";
            return $"{bytes} B";
        }
    }

    public partial class DepotSelectionDialog : Window
    {
        private readonly List<DepotSelectionViewModel> _viewModels;

        public List<string> SelectedDepotIds { get; private set; }

        public DepotSelectionDialog(List<DepotInfo> depots)
        {
            InitializeComponent();

            _viewModels = depots
                .OrderByDescending(d => d.IsMainAppId)
                .ThenBy(d => d.DepotId)
                .Select(d => new DepotSelectionViewModel(d))
                .ToList();

            DepotListBox.ItemsSource = _viewModels;
            SelectedDepotIds = new List<string>();
        }

        public bool IncludeMainAppId { get; private set; } = true;

        private void SelectAll_Click(object sender, RoutedEventArgs e)
        {
            foreach (var vm in _viewModels)
            {
                vm.IsSelected = true;
            }
        }

        private void DeselectAll_Click(object sender, RoutedEventArgs e)
        {
            foreach (var vm in _viewModels)
            {
                vm.IsSelected = false;
            }
        }

        private void Continue_Click(object sender, RoutedEventArgs e)
        {
            var mainAppVm = _viewModels.FirstOrDefault(vm => vm.IsMainAppId);
            IncludeMainAppId = mainAppVm?.IsSelected ?? true;

            SelectedDepotIds = _viewModels
                .Where(vm => vm.IsSelected && !vm.IsMainAppId)
                .Select(vm => vm.DepotId)
                .ToList();

            if (SelectedDepotIds.Count == 0 && !IncludeMainAppId)
            {
                MessageBoxHelper.Show(
                    "Please select at least one depot or the main game to continue.",
                    "Nothing Selected",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
                return;
            }

            var totalCount = SelectedDepotIds.Count + (IncludeMainAppId ? 1 : 0);
            if (totalCount > 128)
            {
                MessageBoxHelper.Show(
                    $"You have selected {totalCount} items, which exceeds GreenLuma's limit of 128. Please deselect some to continue.",
                    "GreenLuma Limit Exceeded",
                    MessageBoxButton.OK,
                    MessageBoxImage.Error);
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
