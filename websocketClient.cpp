#include "websocketClient.hpp"
#include <iostream>
#include <sstream>
#include <boost/asio/connect.hpp>
#include <boost/asio/ip/tcp.hpp>

using namespace std::chrono_literals;

WebSocketClient::WebSocketClient(const std::string& uri)
    : uri_(uri),
      ws_(net::make_strand(ioc_)),
      strand_(ws_.get_executor()) {
    parse_uri();
}

WebSocketClient::~WebSocketClient() {
    disconnect();
}

void WebSocketClient::parse_uri() {
    // Simple URI parser (ws://host:port/path)
    auto pos = uri_.find("://");
    if (pos == std::string::npos) {
        throw std::invalid_argument("Invalid URI");
    }

    auto path_start = uri_.find('/', pos + 3);
    if (path_start == std::string::npos) {
        host_ = uri_.substr(pos + 3);
        path_ = "/";
    } else {
        host_ = uri_.substr(pos + 3, path_start - pos - 3);
        path_ = uri_.substr(path_start);
    }

    auto colon_pos = host_.find(':');
    if (colon_pos != std::string::npos) {
        port_ = host_.substr(colon_pos + 1);
        host_ = host_.substr(0, colon_pos);
    } else {
        port_ = "80";
    }
}

bool WebSocketClient::connect() {
    if (connected_) return true;

    stopping_ = false;
    connect_done_ = false;

    // Start I/O context thread
    ioc_thread_ = std::make_unique<std::thread>([this]() { run_io_context(); });

    // Start connection process
    net::post(strand_, [this]() {
        tcp::resolver resolver(net::make_strand(ioc_));
        resolver.async_resolve(
            host_, port_,
            beast::bind_front_handler(
                &WebSocketClient::on_resolve,
                this
            )
        );
    });

    // Wait for connection with timeout
    std::unique_lock<std::mutex> lock(connect_mutex_);
    if (!connect_cv_.wait_for(lock, 30s, [this] { return connect_done_; })) {
        std::cerr << "Connection timeout" << std::endl;
        disconnect();
        return false;
    }

    return connected_;
}

void WebSocketClient::on_resolve(
    beast::error_code ec,
    tcp::resolver::results_type results) {
    if (ec) {
        std::cerr << "Resolve error: " << ec.message() << std::endl;
        return;
    }

    // Set timeout
    beast::get_lowest_layer(ws_).expires_after(30s);

    // Async connect
    beast::get_lowest_layer(ws_).async_connect(
        results,
        beast::bind_front_handler(
            &WebSocketClient::on_connect,
            this
        )
    );
}

void WebSocketClient::on_connect(
    beast::error_code ec,
    tcp::resolver::results_type::endpoint_type ep) {
    if (ec) {
        std::cerr << "Connect error: " << ec.message() << std::endl;
        return;
    }

    // Disable timeout
    beast::get_lowest_layer(ws_).expires_never();

    // Set suggested timeout settings
    ws_.set_option(
        websocket::stream_base::timeout::suggested(
            beast::role_type::client
        )
    );

    // Set host for WebSocket handshake
    std::string host = host_ + ':' + std::to_string(ep.port());

    // Async handshake
    ws_.async_handshake(
        host, path_,
        beast::bind_front_handler(
            &WebSocketClient::on_handshake,
            this
        )
    );
}

void WebSocketClient::on_handshake(beast::error_code ec) {
    if (ec) {
        std::cerr << "Handshake error: " << ec.message() << std::endl;
        return;
    }

    connected_ = true;
    std::cout << "Connected to " << uri_ << std::endl;

    {
        std::lock_guard<std::mutex> lock(connect_mutex_);
        connect_done_ = true;
    }
    connect_cv_.notify_one();

    start_read();
}

void WebSocketClient::start_read() {
    ws_.async_read(
        read_buffer_,
        beast::bind_front_handler(
            &WebSocketClient::on_read,
            this
        )
    );
}

void WebSocketClient::on_read(
    beast::error_code ec,
    size_t bytes_transferred) {
    if (ec == websocket::error::closed) {
        std::cout << "WebSocket connection closed" << std::endl;
        connected_ = false;
        return;
    } else if (ec) {
        std::cerr << "Read error: " << ec.message() << std::endl;
        connected_ = false;
        return;
    }

    try {
        std::string payload = beast::buffers_to_string(read_buffer_.data());
        read_buffer_.consume(bytes_transferred);
        std::cerr << "Received message: " << payload << std::endl;
        processIncomingMessage(payload);
    } catch (const std::exception& e) {
        std::cerr << "Exception processing message: " << e.what() << std::endl;
    }

    if (connected_) {
        start_read();
    }
}

bool WebSocketClient::sendWebSocketMessage(const std::string& message) {
    if (!connected_) {
        std::cerr << "Not connected to server" << std::endl;
        return false;
    }

    {
        std::lock_guard<std::mutex> lock(write_mutex_);
        write_queue_.push(message);
    }

    net::post(strand_, [this]() {
        if (!writing_) {
            do_write();
        }
    });

    return true;
}

void WebSocketClient::do_write() {
    if (stopping_ || !connected_) return;

    std::lock_guard<std::mutex> lock(write_mutex_);
    if (write_queue_.empty()) {
        writing_ = false;
        return;
    }

    writing_ = true;
    auto message = write_queue_.front();
    write_queue_.pop();

    ws_.async_write(
        net::buffer(message),
        beast::bind_front_handler(
            &WebSocketClient::on_write,
            this
        )
    );
}

void WebSocketClient::on_write(
    beast::error_code ec,
    size_t bytes_transferred) {
    if (ec) {
        std::cerr << "Write error: " << ec.message() << std::endl;
        connected_ = false;
        return;
    }

    std::lock_guard<std::mutex> lock(write_mutex_);
    if (!write_queue_.empty()) {
        do_write();
    } else {
        writing_ = false;
    }
}

void WebSocketClient::disconnect() {
    if (stopping_) return;
    stopping_ = true;

    net::post(strand_, [this]() {
        beast::error_code ec;
        if (ws_.is_open()) {
            ws_.close(websocket::close_code::normal, ec);
            if (ec) {
                std::cerr << "Error closing connection: " << ec.message() << std::endl;
            }
        }
        connected_ = false;
        ioc_.stop();
    });

    if (ioc_thread_ && ioc_thread_->joinable()) {
        ioc_thread_->join();
    }

    // Clear write queue
    std::lock_guard<std::mutex> lock(write_mutex_);
    std::queue<std::string> empty;
    std::swap(write_queue_, empty);
}

void WebSocketClient::run_io_context() {
    try {
        ioc_.run();
    } catch (const std::exception& e) {
        std::cerr << "I/O context error: " << e.what() << std::endl;
    }
}

bool WebSocketClient::isConnected() const {
    return connected_;
}

// Message formatting functions remain unchanged from original implementation
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

// Message processing remains unchanged from original implementation
void WebSocketClient::processIncomingMessage(const std::string& message) {
    std::istringstream stream(message);
    std::string taskId; std::string type; stream >> taskId >> type;;
    if (type == "START") {
        auto tar = message.substr(message.find("START") + 5);
        if (tasks_.contains(taskId)) {
            std::cerr << "Task exists " << taskId << std::endl;
            return;
        }
        Task task(taskId, tar);
        tasks_[taskId] = std::make_shared<Task>(task);
    }
    else if (type == "STOP") {
        if (!tasks_.contains(taskId)) {
            std::cerr << "Task don't exists " << taskId << std::endl;
            return;
        }
        tasks_[taskId]->stop();
    }
}

// Send methods remain unchanged from original implementation
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