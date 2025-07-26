#include "socket.hpp"
#include <cstring>
#include <iostream>
#include <utility>

namespace Socket {
    Connection::Connection(uv_stream_t* s, bool isConnected) : stream(s), connected(isConnected) {
        stream->data = this;
    }

    Connection::~Connection() = default;

    void Connection::write(const char* data, size_t len) const {
        // Calculate total size: 4 bytes for length + data
        uint32_t len_prefix = static_cast<uint32_t>(len);
        size_t total_len = sizeof(len_prefix) + len;
        auto buf = new char[total_len];

        // Copy length into the beginning of the buffer
        std::memcpy(buf, &len_prefix, sizeof(len_prefix));
        // Copy data after the length
        std::memcpy(buf + sizeof(len_prefix), data, len);

        auto* req = new uv_write_t;
        req->data = buf;
        const uv_buf_t uvbuf = uv_buf_init(buf, total_len);
        uv_write(req, stream, &uvbuf, 1, [](uv_write_t* req, int status) {
            if (status < 0) {
                std::cerr << "Write error: " << uv_strerror(status) << std::endl;
            }
            const auto buf = static_cast<char*>(req->data);
            delete[] buf;
            delete req;
        });
    }

    void Connection::write(const std::string& data) const {
        write(data.c_str(), data.size());
    }

    void Connection::close() const {
        uv_close(reinterpret_cast<uv_handle_t*>(stream), [](uv_handle_t* handle) {
            auto* conn = static_cast<Connection*>(handle->data);
            if (conn->closeCallback) {
                conn->closeCallback();
            }
            delete conn;
        });
    }

    void Connection::onData(const std::function<void(const char*, size_t)>& cb) {
        dataCallback = cb;
        if (connected && !dataCallbackSet) {
            dataCallbackSet = true;
            uv_read_start(stream,
                [](uv_handle_t* handle, size_t suggested_size, uv_buf_t* buf) {
                    buf->base = new char[suggested_size];
                    buf->len = suggested_size;
                },
                [](uv_stream_t* stream, ssize_t nread, const uv_buf_t* buf) {
                    auto* conn = static_cast<Connection*>(stream->data);
                    if (nread > 0) {
                        // Add received data to the buffer
                        conn->buffer.insert(conn->buffer.end(), buf->base, buf->base + nread);

                        // Process all complete messages in the buffer
                        while (conn->buffer.size() >= sizeof(uint32_t)) {
                            // Read message length
                            uint32_t len;
                            std::memcpy(&len, conn->buffer.data(), sizeof(uint32_t));

                            // Check if enough data for the full message
                            if (conn->buffer.size() >= sizeof(uint32_t) + len) {
                                // Call callback with the message
                                if (conn->dataCallback) {
                                    conn->dataCallback(conn->buffer.data() + sizeof(uint32_t), len);
                                }
                                // Remove the processed message from the buffer
                                conn->buffer.erase(conn->buffer.begin(), conn->buffer.begin() + sizeof(uint32_t) + len);
                            } else {
                                // Not enough data for a full message, exit the loop
                                break;
                            }
                        }
                    } else if (nread < 0) {
                        uv_close(reinterpret_cast<uv_handle_t*>(stream), [](uv_handle_t* handle) {
                            auto* conn = static_cast<Connection*>(handle->data);
                            if (conn->closeCallback) {
                                conn->closeCallback();
                            }
                            delete conn;
                        });
                    }
                    if (buf->base) {
                        delete[] buf->base;
                    }
                });
        }
    }

    void Connection::onClose(const std::function<void()>& cb) {
        closeCallback = cb;
    }

    void Connection::onConnected(const std::function<void()>& cb) {
        connectedCallback = cb;
        if (connected && connectedCallback) {
            connectedCallback();
        }
    }

    void Server::onNewConnection(uv_stream_t* server, int status) {
        if (status < 0) {
            std::cerr << "New connection error: " << uv_strerror(status) << std::endl;
            return;
        }
        const auto* srv = static_cast<Server*>(server->data);
        auto* client = new uv_pipe_t;
        uv_pipe_init(srv->loop, client, 0);
        if (uv_accept(server, reinterpret_cast<uv_stream_t*>(client)) == 0) {
            auto* conn = new Connection(reinterpret_cast<uv_stream_t*>(client), true);
            if (srv->connectCallback) {
                srv->connectCallback(conn);
            }
        } else {
            uv_close(reinterpret_cast<uv_handle_t*>(client), nullptr);
        }
    }

    Server::Server(const char* path) : socketPath(path) {
        loop = new uv_loop_t;
        uv_loop_init(loop);
        pipe = new uv_pipe_t;
        uv_pipe_init(loop, pipe, 0);
        pipe->data = this;
        uv_pipe_bind(pipe, path);
    }

    Server::~Server() {
        uv_close(reinterpret_cast<uv_handle_t*>(pipe), [](uv_handle_t* handle) {
            delete handle;
        });
        uv_run(loop, UV_RUN_DEFAULT);
        uv_loop_close(loop);
        delete loop;
        unlink(socketPath.c_str());
    }

    void Server::onConnect(const std::function<void(Connection*)>& cb) {
        connectCallback = cb;
    }

    void Server::start(const std::function<void()>& startCallback) const {
        int status = uv_listen(reinterpret_cast<uv_stream_t*>(pipe), 128, onNewConnection);
        if (status == 0) {
            if (startCallback) {
                startCallback();
            }
            uv_run(loop, UV_RUN_DEFAULT);
        } else {
            std::cerr << "Listen error: " << uv_strerror(status) << std::endl;
        }
    }

    void Server::stop() const {
        uv_stop(loop);
    }

    Client::Client() {
        loop = new uv_loop_t;
        uv_loop_init(loop);
    }

    Client::~Client() {
        uv_loop_close(loop);
        delete loop;
    }

    Connection* Client::connect(const char* path) const {
        auto* pipe = new uv_pipe_t;
        uv_pipe_init(loop, pipe, 0);
        auto* conn = new Connection(reinterpret_cast<uv_stream_t*>(pipe), false);
        auto* req = new uv_connect_t;
        req->data = conn;
        uv_pipe_connect(req, pipe, path, [](uv_connect_t* req, int status) {
            auto* conn = static_cast<Connection*>(req->data);
            if (status == 0) {
                conn->connected = true;
                if (conn->connectedCallback) {
                    conn->connectedCallback();
                }
                if (conn->dataCallback) {
                    uv_read_start(conn->stream,
                        [](uv_handle_t* handle, size_t suggested_size, uv_buf_t* buf) {
                            buf->base = new char[suggested_size];
                            buf->len = suggested_size;
                        },
                        [](uv_stream_t* stream, ssize_t nread, const uv_buf_t* buf) {
                            auto* conn = static_cast<Connection*>(stream->data);
                            if (nread > 0) {
                                // Add received data to the buffer
                                conn->buffer.insert(conn->buffer.end(), buf->base, buf->base + nread);

                                // Process all complete messages in the buffer
                                while (conn->buffer.size() >= sizeof(uint32_t)) {
                                    // Read message length
                                    uint32_t len;
                                    std::memcpy(&len, conn->buffer.data(), sizeof(uint32_t));

                                    // Check if enough data for the full message
                                    if (conn->buffer.size() >= sizeof(uint32_t) + len) {
                                        // Call callback with the message
                                        if (conn->dataCallback) {
                                            conn->dataCallback(conn->buffer.data() + sizeof(uint32_t), len);
                                        }
                                        // Remove the processed message from the buffer
                                        conn->buffer.erase(conn->buffer.begin(), conn->buffer.begin() + sizeof(uint32_t) + len);
                                    } else {
                                        // Not enough data for a full message, exit the loop
                                        break;
                                    }
                                }
                            } else if (nread < 0) {
                                uv_close(reinterpret_cast<uv_handle_t*>(stream), [](uv_handle_t* handle) {
                                    auto* conn = static_cast<Connection*>(handle->data);
                                    if (conn->closeCallback) {
                                        conn->closeCallback();
                                    }
                                    delete conn;
                                });
                            }
                            if (buf->base) {
                                delete[] buf->base;
                            }
                        }
                    );
                }
            } else {
                if (conn->closeCallback) {
                    conn->closeCallback();
                }
                uv_close(reinterpret_cast<uv_handle_t*>(conn->stream), [](uv_handle_t* handle) {
                    const auto* conn = static_cast<Connection*>(handle->data);
                    delete conn;
                });
            }
            delete req;
        });
        return conn;
    }

    void Client::stop() const {
        uv_stop(loop);
    }

    void Client::run() const {
        uv_run(loop, UV_RUN_DEFAULT);
    }
}