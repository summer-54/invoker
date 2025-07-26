#include "websocketClient.hpp"
#include <fstream>
#include <iostream>
#include <sstream>
#include <iterator>    // For std::begin, std::end
#include <string_view> // For std::string_view
#include <algorithm>   // For std::search, std::boyer_moore_searcher

void writeStringToFile(const std::string& data, const std::string& filename) {
    std::ofstream file(filename, std::ios::binary);
    if (!file) {
        throw std::runtime_error("Cannot create file: " + filename);
    }

    file.write(data.data(), data.size());

    if (!file) {
        throw std::runtime_error("Error writing to file: " + filename);
    }
}

WebSocketClient::WebSocketClient(const std::string& uri) : uri_(uri), connected_(false), ws_(ioc_) {}

WebSocketClient::~WebSocketClient() {
    disconnect();
}

bool WebSocketClient::connect() {
    try {
        // Parse URI
        size_t pos = uri_.find("://");
        if (pos == std::string::npos) throw std::runtime_error("Invalid URI");
        std::string scheme = uri_.substr(0, pos);
        if (scheme != "ws") throw std::runtime_error("Only ws scheme supported");
        std::string host_port_path = uri_.substr(pos + 3);
        pos = host_port_path.find('/');
        std::string host_port = (pos == std::string::npos) ? host_port_path : host_port_path.substr(0, pos);
        std::string path = (pos == std::string::npos) ? "/" : host_port_path.substr(pos);
        pos = host_port.find(':');
        std::string host = (pos == std::string::npos) ? host_port : host_port.substr(0, pos);
        std::string port = (pos == std::string::npos) ? "80" : host_port.substr(pos + 1);

        // Resolve host
        tcp::resolver resolver(ioc_);
        auto results = resolver.resolve(host, port);

        // Connect socket
        net::connect(ws_.next_layer(), results);

        // Perform handshake
        ws_.handshake(host, path);

        // Set connected flag
        connected_ = true;
        std::cout << "Connected to " + uri_ << std::endl;

        // Start read loop
        startReadLoop();

        // Start io_context in a separate thread
        client_thread_ = std::make_unique<std::thread>([this] { ioc_.run(); });

        return true;
    } catch (const std::exception& e) {
        std::cerr << "Exception during connection: " << e.what() << std::endl;
        return false;
    }
}

void WebSocketClient::disconnect() {
    if (connected_) {
        ws_.async_close(websocket::close_code::normal, [this](beast::error_code ec) {
            if (ec) {
                std::cerr << "Error closing connection: " << ec.message() << std::endl;
            }
            connected_ = false;
        });
    }
    ioc_.stop();
    if (client_thread_ && client_thread_->joinable()) {
        client_thread_->join();
    }
}

bool WebSocketClient::isConnected() const {
    return connected_;
}

void WebSocketClient::startReadLoop() {
    ws_.binary(true); // Set WebSocket to binary mode
    ws_.async_read(buffer_, [this](beast::error_code ec, std::size_t bytes_transferred) {
        if (!ec) {
            // Convert buffer sequence to string and then to vector<char> for binary safety
            std::string data_str = beast::buffers_to_string(buffer_.data());
            std::vector<char> data(data_str.begin(), data_str.end());
            buffer_.consume(bytes_transferred);
            onMessage(std::move(data)); // Pass raw bytes to onMessage
            startReadLoop();
        } else if (ec == websocket::error::closed) {
            onClose();
        } else {
            onFail();
        }
    });
}

void WebSocketClient::onOpen() {
    std::cout << "WebSocket connection opened" << std::endl;
    connected_ = true;
}

void WebSocketClient::onClose() {
    std::cout << "WebSocket connection closed" << std::endl;
    connected_ = false;
}

void WebSocketClient::onMessage(std::vector<char> data) {
    std::cout << "WebSocket message received" << std::endl;
    // Parse taskId and type from the beginning of the data
    auto header_end = std::find(data.begin(), data.end(), '\n');
    if (header_end == data.end()) {
        std::cerr << "Invalid message: no header found" << std::endl;
        return;
    }
    std::string header(data.begin(), data.end());
    std::istringstream stream(header);
    std::string taskId, type;
    stream >> taskId >> type;
    if (type == "START") {
        // Extract binary data after "START\n"
        auto start_pos = std::search(
            data.begin(), data.end(),
            std::boyer_moore_searcher("START\n", "START\n" + 6) // Use const char* instead of sv
        );
        if (start_pos != data.end()) {
            std::vector<char> tar(start_pos + 6, data.end());
            std::string tar_str(tar.begin(), tar.end());
            writeStringToFile(tar_str, "./test0.tar.gz");
            std::lock_guard<std::mutex> lock(tasks_mutex_);
            if (tasks_.contains(taskId)) {
                std::cerr << "Task exists " << taskId << std::endl;
                return;
            }
            tasks_[taskId] = new Task(taskId, tar_str);
        }
    }
}

void WebSocketClient::onFail() {
    std::cerr << "WebSocket connection failed" << std::endl;
    connected_ = false;
}

void WebSocketClient::processIncomingMessage(const std::string& message) {

}

bool WebSocketClient::sendWebSocketMessage(const std::string& message) {
    if (!connected_) {
        std::cerr << "Not connected to server" << std::endl;
        return false;
    }
    net::post(ioc_, [this, message] {
        ws_.async_write(net::buffer(message), [this, message](beast::error_code ec, std::size_t) {
            if (ec) {
                std::cerr << "Error sending message: " << ec.message() << std::endl;
            } else {
                std::cerr << "Sent message: " << message << std::endl;
            }
        });
    });
    return true;
}

std::string WebSocketClient::formatFullVerdictMessage(const std::string& taskId, const std::string& verdict, const std::string& data) {
    std::ostringstream oss;
    oss << taskId << '\n';
    oss << "VERDICT" << ' ' << verdict << '\n';
    oss << data;
    return oss.str();
}

std::string WebSocketClient::formatSubtaskVerdictMessage(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data) {
    std::ostringstream oss;
    oss << taskId << '\n';
    oss << "SUBTASK" << ' ' << subtaskId << '\n';
    oss << "VERDICT" << ' ' << verdict << '\n';
    oss << data;
    return oss.str();
}

std::string WebSocketClient::formatExitedMessage(const std::string& taskId, int exitCode, const std::string& exitData) {
    std::ostringstream oss;
    oss << taskId << '\n';
    oss << "EXITED" << ' ' << exitCode << '\n';
    oss << exitData;
    return oss.str();
}

std::string WebSocketClient::formatErrorMessage(const std::string& taskId, const std::string& errorType, const std::string& errorMessage) {
    std::ostringstream oss;
    oss << taskId << '\n';
    oss << errorType << '\n';
    oss << errorMessage;
    return oss.str();
}

bool WebSocketClient::sendFullVerdict(const std::string& taskId, const std::string& verdict, const std::string& data) {
    std::string message = formatFullVerdictMessage(taskId, verdict, data);
    return sendWebSocketMessage(message);
}

bool WebSocketClient::sendSubtaskVerdict(const std::string& taskId, const std::string& subtaskId, const std::string& verdict, const std::string& data) {
    std::string message = formatSubtaskVerdictMessage(taskId, subtaskId, verdict, data);
    return sendWebSocketMessage(message);
}

bool WebSocketClient::sendExited(const std::string& taskId, int exitCode, const std::string& exitData) {
    std::string message = formatExitedMessage(taskId, exitCode, exitData);
    return sendWebSocketMessage(message);
}

bool WebSocketClient::sendInvokerError(const std::string& taskId, const std::string& errorMessage) {
    std::string message = formatErrorMessage(taskId, "ERROR", errorMessage);
    return sendWebSocketMessage(message);
}

bool WebSocketClient::sendOperatorError(const std::string& taskId, const std::string& errorMessage) {
    std::string message = formatErrorMessage(taskId, "OPERROR", errorMessage);
    return sendWebSocketMessage(message);
}

void WebSocketClient::forEachTask(std::function<void(Task*)> func) {
    std::lock_guard<std::mutex> lock(tasks_mutex_);
    for (auto& pair : tasks_) {
        func(pair.second);
    }
}