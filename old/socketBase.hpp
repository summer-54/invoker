#pragma once

#include <any>
#include <uv.h>
#include <string>
#include <functional>
#include <unordered_map>
#include <iostream>
#include <map>

namespace SocketBase {
    class Server {
    public:
        using ConnectionCallback = std::function<void(uv_stream_t* client)>;
        using DataCallback = std::function<void(uv_stream_t* client, const std::string&)>;

        explicit Server(const std::string& socketPath);
        ~Server();

        void start();
        void stop();
        void setConnectionCallback(const ConnectionCallback& callback);
        void setDataCallback(const DataCallback& callback);
        void write(uv_stream_t* client, const std::string& data);
        void run();

        template<typename T>
        void setConnectionData(uv_stream_t* client, const std::string& key, const T& value) {
            auto it = connections_.find(client);
            if (it != connections_.end()) {
                it->second.data[key] = value;
            }
        }

        template<typename T>
        T getConnectionData(uv_stream_t* client, const std::string& key) {
            auto it = connections_.find(client);
            if (it != connections_.end()) {
                try {
                    return std::any_cast<T>(it->second.data.at(key));
                } catch (const std::bad_any_cast&) {
                    throw std::runtime_error("Invalid type requested for connection data");
                } catch (const std::out_of_range&) {
                    throw std::runtime_error("Key not found in connection data");
                }
            }
            throw std::runtime_error("Client not found");
        }

        void removeConnectionData(uv_stream_t* client, const std::string& key);
        void clearConnectionData(uv_stream_t* client);

    private:
        struct ClientContext {
            uv_pipe_t* handle;
            Server* server;
            std::map<std::string, std::any> data; // Store arbitrary data for each connection
        };

        static constexpr int DEFAULT_BACKLOG = 128;

        std::string socketPath_;
        uv_loop_t* loop_;
        uv_pipe_t server_;
        std::unordered_map<uv_stream_t*, ClientContext> connections_;
        ConnectionCallback connectionCallback_;
        DataCallback dataCallback_;

        void removeSocketFile();
        void onNewConnection(int status);
        void onDataReceived(uv_stream_t* client, ssize_t nread, const uv_buf_t* buf);
    };

    class Client {
    public:
        using ConnectCallback = std::function<void(bool success)>;
        using DataCallback = std::function<void(const std::string&)>;

        Client(const std::string& socketPath);
        ~Client();

        void connect(ConnectCallback callback);
        void disconnect();
        void setDataCallback(DataCallback callback);
        void write(const std::string& data);
        void run();

    private:
        std::string socketPath_;
        uv_loop_t* loop_;
        uv_pipe_t socket_;
        uv_connect_t connectReq_;
        ConnectCallback connectCallback_;
        DataCallback dataCallback_;

        void onConnected(int status);
        void onDataReceived(ssize_t nread, const uv_buf_t* buf);
    };
}