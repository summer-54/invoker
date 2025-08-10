#pragma once
#include <boost/process.hpp>
#include <boost/asio.hpp>
#include <functional>
#include <string>
#include <thread>
#include <memory>

class AsyncProcess {
    mutable boost::process::child childProcess;
    boost::asio::io_context ioContext;
    boost::asio::executor_work_guard<boost::asio::io_context::executor_type> work;
    std::thread ioThread;

    boost::process::async_pipe outPipe;
    boost::process::async_pipe errPipe;

    std::array<char, 4096> outBuffer{};
    std::array<char, 4096> errBuffer{};

    std::function<void(const std::string&)> stdoutCallback;
    std::function<void(const std::string&)> stderrCallback;
    std::function<void(int)> endCallback;

    void readOutput();
    void readErrors();
    void checkCompletion() const;

public:
    boost::process::opstream in;

    void start(const std::string& command, const std::string& cwd = "");
    explicit AsyncProcess(const std::string& command, const std::string& cwd = "");
    AsyncProcess();
    ~AsyncProcess();
    void onStdout(const std::function<void(const std::string&)>& callback);
    void onStderr(const std::function<void(const std::string&)>& callback);
    void onEnd(const std::function<void(int)>& callback);
    void terminate() const;
    bool running() const;
    void write(const std::string& str);
};