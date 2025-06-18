#pragma once
#include <vector>
#include <string>
#include "socketBase.hpp"

namespace Socket {
    class DataBuffer {
    public:
        using DataCallback = std::function<void(const std::string&)>;

    private:
        // std::vector<const std::string&> dataQueue;
        DataCallback dataCallback_;
        bool flag = true;
        std::vector<std::string> queue;

    public:
        void emit(const std::string& data) const;
        void on(const DataCallback& callback);
    };

    class Socket {
    public:
        using DataCallback = std::function<void(const std::string&)>;

    private:
        std::function<void(const std::string&)> write_;
        std::vector<DataCallback> dataCallbacks_;

    public:
        Socket(SocketBase::Server& server, uv_stream_t* client, DataBuffer& dataBuffer);
        explicit Socket(SocketBase::Client& client, DataBuffer& dataBuffer);

        void write(const std::string& data) const;
        void onData(const DataCallback& callback);
    };

    void serve(const std::string& socketPath, std::function<void(Socket&)> callback);

    void connect(const std::string& socketPath, std::function<void(Socket&)> callback);
}