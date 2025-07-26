#ifndef SOCKET_HPP
#define SOCKET_HPP

#include <uv.h>
#include <functional>
#include <string>
#include <vector>  // Added for buffer

namespace Socket {
    class Connection {
    public:
        uv_stream_t* stream;
        bool connected, dataCallbackSet = false;
        std::function<void(const char*, size_t)> dataCallback;
        std::function<void()> closeCallback;
        std::function<void()> connectedCallback;
        std::vector<char> buffer;  // Added for accumulating incoming data
        void* data = nullptr;

        Connection(uv_stream_t* s, bool isConnected);
        ~Connection();

        void write(const char* data, size_t len) const;
        void write(const std::string& data) const;
        void close() const;
        void onData(const std::function<void(const char*, size_t)>& cb);
        void onClose(const std::function<void()>& cb);
        void onConnected(const std::function<void()>& cb);
    };

    class Server {
    private:
        uv_loop_t* loop;
        uv_pipe_t* pipe;
        std::string socketPath;
        std::function<void(Connection*)> connectCallback;

        static void onNewConnection(uv_stream_t* server, int status);

    public:
        explicit Server(const char* path);
        ~Server();

        void onConnect(const std::function<void(Connection*)>& cb);
        void start(const std::function<void()>& startCallback) const;
        void stop() const;
    };

    class Client {
    private:
        uv_loop_t* loop;

    public:
        Client();
        ~Client();

        Connection* connect(const char* path) const;
        void run() const;
        void stop() const;
    };
}

#endif // SOCKET_HPP