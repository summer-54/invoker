#include "socket.hpp"
#include <memory>

void Socket::DataBuffer::emit(const std::string& data) const {
    dataCallback_(data);
}

void Socket::DataBuffer::on(const DataCallback& callback) {
    dataCallback_ = callback;
    if (flag) {
        for (const auto& str : queue) {
            dataCallback_(str);
        }
        flag = false;
        queue.clear();
    }
}

Socket::Socket::Socket(SocketBase::Server& server, uv_stream_t* client, DataBuffer& dataBuffer) {
    write_ = [&server, &client](const std::string& data) {
        server.write(client, data);
    };
    dataBuffer.on([this](const std::string& data) {
        for (auto callback : dataCallbacks_) {
            callback(data);
        }
    });
}

Socket::Socket::Socket(SocketBase::Client& client, DataBuffer& dataBuffer) {
    write_ = [&client](const std::string& data) {
        client.write(data);
    };
    dataBuffer.on([this](const std::string& data) {
        for (auto callback : dataCallbacks_) {
            callback(data);
        }
    });
}

void Socket::Socket::write(const std::string& data) const {
    write_(data);
}

void Socket::Socket::onData(const DataCallback& callback) {
    dataCallbacks_.push_back(callback);
}

void Socket::serve(const std::string& socketPath, std::function<void(Socket&)> callback) {
    SocketBase::Server server(socketPath);
    server.setDataCallback([&server](uv_stream_t* client, const std::string& data) {
        try {
            DataBuffer& dataBuffer = server.getConnectionData<DataBuffer&>(client, "dataBuffer");
            dataBuffer.emit(data);
        } catch (const std::exception& e) {
            std::cerr << "Error in data callback: " << e.what() << std::endl;
        }
    });
    server.setConnectionCallback([&callback, &server](uv_stream_t* client) {
        server.setConnectionData<DataBuffer>(client, "dataBuffer", DataBuffer());
        DataBuffer& dataBuffer = server.getConnectionData<DataBuffer&>(client, "dataBuffer");

        auto socketPtr = std::make_shared<Socket>(server, client, dataBuffer);
        server.setConnectionData<std::shared_ptr<Socket>>(client, "socket", socketPtr);

        callback(*socketPtr);
    });
    server.start();
    server.run();
}

void Socket::connect(const std::string& socketPath, std::function<void(Socket&)> callback) {
    auto client = std::make_shared<SocketBase::Client>(socketPath);
    auto dataBuffer = std::make_shared<DataBuffer>();
    auto socket = std::make_shared<Socket>(*client, *dataBuffer);

    client->setDataCallback([dataBuffer](const std::string& data) {
        dataBuffer->emit(data);
    });

    client->connect([client, dataBuffer, socket, callback](bool success) {
        if (success) {
            callback(*socket);
        } else {
            std::cerr << "Failed to connect to server" << std::endl;
        }
    });

    // Сохраняем shared_ptr в цикле событий
    uv_default_loop()->data = new std::tuple<
        std::shared_ptr<SocketBase::Client>,
        std::shared_ptr<DataBuffer>,
        std::shared_ptr<Socket>
    >(client, dataBuffer, socket);

    client->run();

    // Освобождаем ресурсы после завершения цикла
    auto* data = static_cast<std::tuple<
        std::shared_ptr<SocketBase::Client>,
        std::shared_ptr<DataBuffer>,
        std::shared_ptr<Socket>
    >*>(uv_default_loop()->data);
    delete data;
    uv_default_loop()->data = nullptr;
}