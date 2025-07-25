#ifndef INVOKER_WEBSOCKETCLIENT_H
#define INVOKER_WEBSOCKETCLIENT_H

#include <boost/beast/websocket.hpp>
#include <boost/beast/core.hpp>
#include <boost/asio/strand.hpp>
#include <boost/asio/steady_timer.hpp>
#include <string>
#include <memory>
#include <atomic>
#include <thread>
#include <mutex>
#include <condition_variable>
#include <queue>
#include <map>

#include "task.hpp"

namespace beast = boost::beast;
namespace websocket = beast::websocket;
namespace net = boost::asio;
using tcp = net::ip::tcp;

class WebSocketClient {
public:
    std::map<std::string, std::shared_ptr<Task>> tasks_;

    explicit WebSocketClient(const std::string& uri);
    ~WebSocketClient();

    bool connect();
    void disconnect();
    [[nodiscard]] bool isConnected() const;

    // Send methods
    bool sendFullVerdict(const std::string& taskId, const std::string& verdict, const std::string& data);
    bool sendSubtaskVerdict(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data);
    bool sendExited(const std::string& taskId, int exitCode, const std::string& exitData);
    bool sendInvokerError(const std::string& taskId, const std::string& errorMessage);
    bool sendOperatorError(const std::string& taskId, const std::string& errorMessage);

private:
    std::string uri_;
    std::string host_;
    std::string port_;
    std::string path_;

    net::io_context ioc_;
    websocket::stream<beast::tcp_stream> ws_;
    net::strand<websocket::stream<beast::tcp_stream>::executor_type> strand_;
    std::atomic<bool> connected_{false};
    std::atomic<bool> stopping_{false};
    std::unique_ptr<std::thread> ioc_thread_;

    // Connection synchronization
    std::mutex connect_mutex_;
    std::condition_variable connect_cv_;
    bool connect_done_{false};

    // Read buffer
    beast::flat_buffer read_buffer_;

    // Write queue
    std::mutex write_mutex_;
    std::queue<std::string> write_queue_;
    bool writing_{false};

    void parse_uri();
    void on_resolve(beast::error_code ec, tcp::resolver::results_type results);
    void on_connect(beast::error_code ec, tcp::resolver::results_type::endpoint_type ep);
    void on_handshake(beast::error_code ec);
    void start_read();
    void on_read(beast::error_code ec, size_t bytes_transferred);
    void on_write(beast::error_code ec, size_t bytes_transferred);
    void do_write();
    void run_io_context();

    // Message processing and formatting
    void processIncomingMessage(const std::string& message);
    bool sendWebSocketMessage(const std::string& message);
    static std::string formatFullVerdictMessage(const std::string& taskId, const std::string& verdict, const std::string& data);
    static std::string formatSubtaskVerdictMessage(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data);
    static std::string formatExitedMessage(const std::string& taskId, int exitCode, const std::string& exitData);
    static std::string formatErrorMessage(const std::string& taskId, const std::string& errorType, const std::string& errorMessage);
};

#endif // INVOKER_WEBSOCKETCLIENT_H