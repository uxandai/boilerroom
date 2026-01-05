using SolusManifestApp.ViewModels;
using System.Windows.Controls;

namespace SolusManifestApp.Views
{
    public partial class StorePage : UserControl
    {
        public StorePage()
        {
            InitializeComponent();
            DataContextChanged += StorePage_DataContextChanged;
        }

        private void StorePage_DataContextChanged(object sender, System.Windows.DependencyPropertyChangedEventArgs e)
        {
            if (e.NewValue is StoreViewModel viewModel)
            {
                viewModel.ScrollToTopAction = ScrollToTop;
            }
        }

        public void ScrollToTop()
        {
            StoreScrollViewer.ScrollToTop();
        }
    }
}
