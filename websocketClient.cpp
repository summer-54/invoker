// #include "websocketClient.hpp"
// #include <iostream>
// #include <sstream>
//
// WebSocketClient::WebSocketClient(const std::string& uri): uri_(uri), connected_(false) {
//
//     // Инициализация клиента WebSocket
//     client_.clear_access_channels(websocketpp::log::alevel::all);
//     client_.clear_error_channels(websocketpp::log::elevel::all);
//
//     // Установка обработчиков событий
//     client_.set_open_handler(websocketpp::lib::bind(&WebSocketClient::onOpen, this, websocketpp::lib::placeholders::_1));
//     client_.set_close_handler(websocketpp::lib::bind(&WebSocketClient::onClose, this, websocketpp::lib::placeholders::_1));
//     client_.set_message_handler(websocketpp::lib::bind(&WebSocketClient::onMessage, this,
//         websocketpp::lib::placeholders::_1, websocketpp::lib::placeholders::_2));
//     client_.set_fail_handler(websocketpp::lib::bind(&WebSocketClient::onFail, this, websocketpp::lib::placeholders::_1));
//
//     // Инициализация asio
//     client_.init_asio();
// }
//
// WebSocketClient::~WebSocketClient() {
//     disconnect();
// }
//
// bool WebSocketClient::connect() {
//     try {
//         websocketpp::lib::error_code ec;
//         client::connection_ptr con = client_.get_connection(uri_, ec);
//
//         if (ec) {
//             std::cerr << ("Could not create connection: " + ec.message());
//             return false;
//         }
//
//         connection_hdl_ = con->get_handle();
//         client_.connect(con);
//
//         // Запуск io_service в отдельном потоке
//         client_thread_ = std::make_unique<std::thread>(&client::run, &client_);
//
//         // Ожидание подключения (с таймаутом)
//         int timeout = 30;
//         while (!connected_ && timeout > 0) {
//             std::this_thread::sleep_for(std::chrono::seconds(1));
//             timeout--;
//         }
//
//         if (!connected_) {
//             std::cerr << ("Connection timeout");
//             return false;
//         }
//
//         std::cout << ("Connected to " + uri_);
//         return true;
//     } catch (const std::exception& e) {
//         std::cerr << ("Exception during connection: " + std::string(e.what()));
//         return false;
//     }
// }
//
// void WebSocketClient::disconnect() {
//     if (connected_) {
//         websocketpp::lib::error_code ec;
//         client_.close(connection_hdl_, websocketpp::close::status::normal, "Goodbye", ec);
//         if (ec) {
//             std::cerr << ("Error closing connection: " + ec.message());
//         }
//         connected_ = false;
//     }
//
//     if (client_thread_ && client_thread_->joinable()) {
//         client_thread_->join();
//     }
// }
//
// bool WebSocketClient::isConnected() const {
//     return connected_;
// }
//
// bool WebSocketClient::sendFullVerdict(const std::string& taskId, const std::string& verdict, const std::string& data) {
//     std::string message = formatFullVerdictMessage(taskId, verdict, data);
//     return sendWebSocketMessage(message);
// }
//
// bool WebSocketClient::sendSubtaskVerdict(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data) {
//     std::string message = formatSubtaskVerdictMessage(taskId, subtaskId, verdict, data);
//     return sendWebSocketMessage(message);
// }
//
// bool WebSocketClient::sendExited(const std::string& taskId, int exitCode, const std::string& exitData) {
//     std::string message = formatExitedMessage(taskId, exitCode, exitData);
//     return sendWebSocketMessage(message);
// }
//
// bool WebSocketClient::sendInvokerError(const std::string& taskId, const std::string& errorMessage) {
//     std::string message = formatErrorMessage(taskId, "ERROR", errorMessage);
//     return sendWebSocketMessage(message);
// }
//
// bool WebSocketClient::sendOperatorError(const std::string& taskId, const std::string& errorMessage) {
//     std::string message = formatErrorMessage(taskId, "OPERROR", errorMessage);
//     return sendWebSocketMessage(message);
// }
//
// void WebSocketClient::onOpen(websocketpp::connection_hdl hdl) {
//     std::cout << ("WebSocket connection opened");
//     connected_ = true;
// }
//
// void WebSocketClient::onClose(websocketpp::connection_hdl hdl) {
//     std::cout << ("WebSocket connection closed");
//     connected_ = false;
// }
//
// void WebSocketClient::onMessage(websocketpp::connection_hdl hdl, message_ptr msg) {
//     try {
//         std::string payload = msg->get_payload();
//         std::cerr << ("Received message: " + payload);
//         processIncomingMessage(payload);
//     } catch (const std::exception& e) {
//         std::cerr << ("Exception processing message: " + std::string(e.what()));
//     }
// }
//
// void WebSocketClient::onFail(websocketpp::connection_hdl hdl) {
//     std::cerr << ("WebSocket connection failed");
//     connected_ = false;
// }
//
// void WebSocketClient::processIncomingMessage(const std::string& message) {
//     std::istringstream stream(message);
//     std::string taskId; std::string type; stream >> taskId >> type;;
//     if (type == "START") {
//         auto tar = message.substr(message.find("START") + 5);
//         if (tasks_.contains(taskId)) {
//             std::cerr << "Task exists " << taskId << std::endl;
//             return;
//         }
//         tasks_[taskId] = new Task(taskId, tar);
//     }
//     else if (type == "STOP") {
//         if (!tasks_.contains(taskId)) {
//             std::cerr << "Task don't exists " << taskId << std::endl;
//             return;
//         }
//         tasks_[taskId]->stop();
//     }
// }
//
// bool WebSocketClient::sendWebSocketMessage(const std::string& message) {
//     if (!connected_) {
//         std::cerr << ("Not connected to server");
//         return false;
//     }
//
//     try {
//         client_.send(connection_hdl_, message, websocketpp::frame::opcode::text);
//         std::cerr << ("Sent message: " + message);
//         return true;
//     } catch (const std::exception& e) {
//         std::cerr << ("Exception sending message: " + std::string(e.what()));
//         return false;
//     }
// }
//
// std::string WebSocketClient::formatFullVerdictMessage(const std::string& taskId, const std::string& verdict, const std::string& data) {
//     std::ostringstream oss;
//     oss << taskId << '\n';
//     oss << "VERDICT" << ' ' << verdict << '\n';
//     oss << data;
//     return oss.str();
// }
//
// std::string WebSocketClient::formatSubtaskVerdictMessage(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data) {
//     std::ostringstream oss;
//     oss << taskId << '\n';
//     oss << "SUBTASK" << ' ' << subtaskId << '\n';
//     oss << "VERDICT" << ' ' << verdict << '\n';
//     oss << data;
//     return oss.str();
// }
//
// std::string WebSocketClient::formatExitedMessage(const std::string& taskId, int exitCode, const std::string& exitData) {
//     std::ostringstream oss;
//     oss << taskId << '\n';
//     oss << "EXITED" << ' ' << exitCode << '\n';
//     oss << exitData;
//     return oss.str();
// }
//
// std::string WebSocketClient::formatErrorMessage(const std::string& taskId, const std::string& errorType, const std::string& errorMessage) {
//     std::ostringstream oss;
//     oss << taskId << '\n';
//     oss << errorType << '\n';
//     oss << errorMessage;
//     return oss.str();
// }