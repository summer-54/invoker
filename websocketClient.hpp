// #ifndef INVOKER_WEBSOCKETCLIENT_H
// #define INVOKER_WEBSOCKETCLIENT_H
//
// // #include <websocketpp/config/asio_no_tls_client.hpp>
// // #include <websocketpp/client.hpp>
// // #include <websocketpp/common/thread.hpp>
// // #include <websocketpp/common/memory.hpp>
//
// #include <string>
// #include <memory>
// #include <atomic>
//
// #include "session.hpp"
// #include "task.hpp"
//
// class WebSocketClient {
// public:
//     using client = websocketpp::client<websocketpp::config::asio_client>;
//     using message_ptr = websocketpp::config::asio_client::message_type::ptr;
//     using connection_ptr = websocketpp::connection_hdl;
//
//     std::map<std::string, Task*> tasks_;
//
//     explicit WebSocketClient(const std::string& uri);
//     ~WebSocketClient();
//
//     bool connect();
//     void disconnect();
//     bool isConnected() const;
//
//     // Отправка сообщений серверу
//     bool sendFullVerdict(const std::string& taskId, const std::string& verdict, const std::string& data);
//     bool sendSubtaskVerdict(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data);
//     bool sendExited(const std::string& taskId, int exitCode, const std::string& exitData);
//     bool sendInvokerError(const std::string& taskId, const std::string& errorMessage);
//     bool sendOperatorError(const std::string& taskId, const std::string& errorMessage);
//
// private:
//     std::string uri_;
//     client client_;
//     websocketpp::connection_hdl connection_hdl_;
//     std::atomic<bool> connected_;
//     std::unique_ptr<std::thread> client_thread_;
//
//     // Обработчики событий WebSocket
//     void onOpen(websocketpp::connection_hdl hdl);
//     void onClose(websocketpp::connection_hdl hdl);
//     void onMessage(websocketpp::connection_hdl hdl, message_ptr msg);
//     void onFail(websocketpp::connection_hdl hdl);
//
//     // Обработка входящих сообщений
//     void processIncomingMessage(const std::string& message);
//
//     // Отправка сообщения через WebSocket
//     bool sendWebSocketMessage(const std::string& message);
//
//     // Форматирование сообщений
//     static std::string formatFullVerdictMessage(const std::string& taskId, const std::string& verdict, const std::string& data);
//     static std::string formatSubtaskVerdictMessage(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data);
//     static std::string formatExitedMessage(const std::string& taskId, int exitCode, const std::string& exitData);
//     static std::string formatErrorMessage(const std::string& taskId, const std::string& errorType, const std::string& errorMessage);
// };
//
// #endif // INVOKER_WEBSOCKETCLIENT_H