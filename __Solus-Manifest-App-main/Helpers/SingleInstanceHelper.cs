using System;
using System.Threading;
using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Threading.Tasks;

namespace SolusManifestApp.Helpers
{
    public class SingleInstanceHelper : IDisposable
    {
        private const string MutexName = "SolusManifestApp_SingleInstance_Mutex";
        private const string PipeName = "SolusManifestApp_IPC_Pipe";
        private Mutex? _mutex;
        private NamedPipeServerStream? _pipeServer;
        private bool _isFirstInstance;

        public event EventHandler<string>? ArgumentsReceived;

        public bool IsFirstInstance => _isFirstInstance;

        public bool TryAcquire()
        {
            try
            {
                _mutex = new Mutex(true, MutexName, out _isFirstInstance);

                if (_isFirstInstance)
                {
                    // Start listening for messages from other instances
                    StartPipeServer();
                }

                return _isFirstInstance;
            }
            catch
            {
                return false;
            }
        }

        public static void SendArgumentsToFirstInstance(string arguments)
        {
            try
            {
                using var client = new NamedPipeClientStream(".", PipeName, PipeDirection.Out);
                client.Connect(1000); // 1 second timeout

                var data = Encoding.UTF8.GetBytes(arguments);
                client.Write(data, 0, data.Length);
                client.Flush();
            }
            catch
            {
                // Failed to send to first instance
            }
        }

        private void StartPipeServer()
        {
            Task.Run(async () =>
            {
                while (_isFirstInstance)
                {
                    try
                    {
                        _pipeServer = new NamedPipeServerStream(
                            PipeName,
                            PipeDirection.In,
                            1,
                            PipeTransmissionMode.Byte,
                            PipeOptions.Asynchronous);

                        await _pipeServer.WaitForConnectionAsync();

                        using var reader = new StreamReader(_pipeServer, Encoding.UTF8);
                        var message = await reader.ReadToEndAsync();

                        if (!string.IsNullOrEmpty(message))
                        {
                            ArgumentsReceived?.Invoke(this, message);
                        }

                        _pipeServer.Disconnect();
                        _pipeServer.Dispose();
                    }
                    catch
                    {
                        // Pipe error, restart
                        _pipeServer?.Dispose();
                    }
                }
            });
        }

        public void Dispose()
        {
            _isFirstInstance = false;
            _pipeServer?.Dispose();
            _mutex?.Dispose();
        }
    }
}
