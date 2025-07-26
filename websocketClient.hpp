#ifndef INVOKER_WEBSOCKETCLIENT_H
#define INVOKER_WEBSOCKETCLIENT_H

#include <boost/beast.hpp>
#include <boost/asio.hpp>
#include <string>
#include <map>
#include <memory>
#include <atomic>
#include <mutex>
#include <functional>

#include "session.hpp"
#include "task.hpp"

namespace beast = boost::beast;
namespace websocket = beast::websocket;
namespace net = boost::asio;
using tcp = net::ip::tcp;

class WebSocketClient {
public:
    explicit WebSocketClient(const std::string& uri);
    ~WebSocketClient();

    bool connect();
    void disconnect();
    bool isConnected() const;

    // Sending methods
    bool sendFullVerdict(const std::string& taskId, const std::string& verdict, const std::string& data);
    bool sendSubtaskVerdict(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data);
    bool sendExited(const std::string& taskId, int exitCode, const std::string& exitData);
    bool sendInvokerError(const std::string& taskId, const std::string& errorMessage);
    bool sendOperatorError(const std::string& taskId, const std::string& errorMessage);

    // Helper method for thread-safe task iteration
    void forEachTask(std::function<void(Task*)> func);

    std::map<std::string, Task*> tasks_;

private:
    std::string uri_;
    net::io_context ioc_;
    websocket::stream<tcp::socket> ws_;
    beast::flat_buffer buffer_;
    std::atomic<bool> connected_;
    std::unique_ptr<std::thread> client_thread_;
    std::mutex tasks_mutex_;

    // Handlers
    void onOpen();
    void onClose();
    void onMessage(std::vector<char> data);
    void onFail();

    // Message processing
    void processIncomingMessage(const std::string& message);

    // Sending
    bool sendWebSocketMessage(const std::string& message);

    // Message formatting
    static std::string formatFullVerdictMessage(const std::string& taskId, const std::string& verdict, const std::string& data);
    static std::string formatSubtaskVerdictMessage(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data);
    static std::string formatExitedMessage(const std::string& taskId, int exitCode, const std::string& exitData);
    static std::string formatErrorMessage(const std::string& taskId, const std::string& errorType, const std::string& errorMessage);

    // Read loop
    void startReadLoop();
};

#endif // INVOKER_WEBSOCKETCLIENT_H