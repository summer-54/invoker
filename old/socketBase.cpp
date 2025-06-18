#include <memory>
#include "socketBase.hpp"

namespace SocketBase {
    // UnixSocketServer implementation
    Server::Server(const std::string& socketPath)
        : socketPath_(socketPath), loop_(uv_default_loop()) {}

    Server::~Server() {
        stop();
    }

    void Server::start() {
        removeSocketFile();

        uv_pipe_init(loop_, &server_, 0);

        if (int r = uv_pipe_bind(&server_, socketPath_.c_str())) {
            throw std::runtime_error("Bind error: " + std::string(uv_strerror(r)));
        }

        server_.data = this;
        if (int r = uv_listen((uv_stream_t*)&server_, DEFAULT_BACKLOG,
                             [](uv_stream_t* server, int status) {
            static_cast<Server*>(server->data)->onNewConnection(status);
        })) {
            throw std::runtime_error("Listen error: " + std::string(uv_strerror(r)));
        }

        std::cout << "Server listening on Unix socket: " << socketPath_ << std::endl;
    }

    void Server::stop() {
        uv_close((uv_handle_t*)&server_, nullptr);
        for (auto& [handle, ctx] : connections_) {
            uv_close((uv_handle_t*)handle, [](uv_handle_t* handle) {
                delete (uv_pipe_t*)handle;
            });
        }
        connections_.clear();
    }

    void Server::setConnectionCallback(const ConnectionCallback& callback) {
        connectionCallback_ = callback;
    }

    void Server::setDataCallback(const DataCallback& callback) {
        dataCallback_ = callback;
    }

    void Server::write(uv_stream_t* client, const std::string& data) {
        auto* buf = new char[data.size()];
        std::copy(data.begin(), data.end(), buf);

        uv_write_t* req = new uv_write_t;
        req->data = buf;

        uv_buf_t uvBuf = uv_buf_init(buf, data.size());
        std::cerr << data << '\n';

        uv_write(req, client, &uvBuf, 1, [](uv_write_t* req, int status) {
            delete[] static_cast<char*>(req->data);
            delete req;
            std::cerr << "test" << '\n';

            if (status) {
                std::cerr << "Write error: " << uv_strerror(status) << std::endl;
            }
        });
    }

    void Server::run() {
        uv_run(loop_, UV_RUN_DEFAULT);
    }

    void Server::removeConnectionData(uv_stream_t* client, const std::string& key) {
        auto it = connections_.find(client);
        if (it != connections_.end()) {
            it->second.data.erase(key);
        }
    }

    void Server::clearConnectionData(uv_stream_t* client) {
        auto it = connections_.find(client);
        if (it != connections_.end()) {
            it->second.data.clear();
        }
    }

    void Server::removeSocketFile() {
        uv_fs_t req;
        uv_fs_unlink(loop_, &req, socketPath_.c_str(), nullptr);
        uv_fs_req_cleanup(&req);
    }

    void Server::onNewConnection(int status) {
        if (status < 0) {
            std::cerr << "New connection error: " << uv_strerror(status) << std::endl;
            return;
        }

        auto* client = new uv_pipe_t;
        uv_pipe_init(loop_, client, 0);

        if (uv_accept((uv_stream_t*)&server_, (uv_stream_t*)client) == 0) {
            ClientContext ctx{client, this};
            connections_.emplace((uv_stream_t*)client, ctx);

            client->data = &connections_.at((uv_stream_t*)client);

            // Cast the client to uv_stream_t* before passing to the callback
            connectionCallback_((uv_stream_t*)client);

            uv_read_start((uv_stream_t*)client,
                [](uv_handle_t* handle, size_t size, uv_buf_t* buf) {
                    buf->base = new char[size];
                    buf->len = size;
                },
                [](uv_stream_t* stream, ssize_t nread, const uv_buf_t* buf) {
                    auto* ctx = static_cast<ClientContext*>(stream->data);
                    ctx->server->onDataReceived(stream, nread, buf);
                });
        } else {
            delete client;
        }
    }

    void Server::onDataReceived(uv_stream_t* client, ssize_t nread, const uv_buf_t* buf) {
        std::unique_ptr<char[]> bufferGuard(buf->base);

        if (nread > 0) {
            std::string data(buf->base, nread);
            if (dataCallback_) {
                try {
                    dataCallback_(client, data);
                } catch (const std::exception& e) {
                    std::cerr << "Exception in data callback: " << e.what() << std::endl;
                }
            }
        }

        if (nread < 0) {
            if (nread != UV_EOF) {
                std::cerr << "Read error: " << uv_err_name(nread) << std::endl;
            }
            uv_close((uv_handle_t*)client, [](uv_handle_t* handle) {
                auto* ctx = static_cast<ClientContext*>(handle->data);
                if (ctx) {
                    ctx->server->connections_.erase((uv_stream_t*)handle);
                }
                delete (uv_pipe_t*)handle;
            });
        }
    }

    // UnixSocketClient implementation
    Client::Client(const std::string& socketPath)
        : socketPath_(socketPath), loop_(uv_default_loop()) {}

    Client::~Client() {
        disconnect();
    }

    void Client::connect(ConnectCallback callback) {
        uv_pipe_init(loop_, &socket_, 0);
        socket_.data = this;

        connectCallback_ = callback;

        uv_pipe_connect(&connectReq_, &socket_, socketPath_.c_str(),
            [](uv_connect_t* req, int status) {
                auto* self = static_cast<Client*>(req->handle->data);
                self->onConnected(status);
            });
    }

    void Client::disconnect() {
        if (uv_is_active((uv_handle_t*)&socket_)) {
            uv_close((uv_handle_t*)&socket_, [](uv_handle_t* handle) {
                // Optional: Add any cleanup here
            });
        }
    }

    void Client::setDataCallback(DataCallback callback) {
        dataCallback_ = callback;
    }

    void Client::write(const std::string& data) {
        auto* writeReq = new uv_write_t;
        auto* buffer = new char[data.size()];
        std::copy(data.begin(), data.end(), buffer);

        uv_buf_t buf = uv_buf_init(buffer, data.size());
        writeReq->data = buffer;

        uv_write(writeReq, (uv_stream_t*)&socket_, &buf, 1,
            [](uv_write_t* req, int status) {
                delete[] static_cast<char*>(req->data);
                delete req;

                if (status) {
                    std::cerr << "Write error: " << uv_strerror(status) << std::endl;
                }
            });
    }

    void Client::run() {
        uv_run(loop_, UV_RUN_DEFAULT);
    }

    void Client::onConnected(int status) {
        if (connectCallback_) {
            connectCallback_(status == 0);
        }

        if (status == 0) {
            uv_read_start((uv_stream_t*)&socket_,
                [](uv_handle_t* handle, size_t size, uv_buf_t* buf) {
                    buf->base = new char[size];
                    buf->len = size;
                },
                [](uv_stream_t* stream, ssize_t nread, const uv_buf_t* buf) {
                    auto* self = static_cast<Client*>(stream->data);
                    self->onDataReceived(nread, buf);
                });
        } else {
            std::cerr << "Connection error: " << uv_strerror(status) << std::endl;
        }
    }

    void Client::onDataReceived(ssize_t nread, const uv_buf_t* buf) {
        std::unique_ptr<char[]> bufferGuard(buf->base);

        if (nread > 0) {
            std::string data(buf->base, nread);
            if (dataCallback_) {
                dataCallback_(data);
            } else {
                std::cout << "Received: " << data << std::endl;
            }
        }

        if (nread < 0) {
            if (nread != UV_EOF) {
                std::cerr << "Read error: " << uv_err_name(nread) << std::endl;
            }
            disconnect();
        }
    }
}