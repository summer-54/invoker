#include "asyncProcess.hpp"
#include <iostream>
#include <boost/filesystem.hpp>

void AsyncProcess::readOutput() {
    outPipe.async_read_some(boost::asio::buffer(outBuffer),
    [this](const boost::system::error_code& ec, std::size_t size) {
        if (!ec && size > 0 && stdoutCallback) {
            std::string data(outBuffer.data(), size);
            stdoutCallback(data);
            readOutput();
        } else if (ec == boost::asio::error::eof) {
            checkCompletion();
        }
    });
}

void AsyncProcess::readErrors() {
    errPipe.async_read_some(boost::asio::buffer(errBuffer),
    [this](const boost::system::error_code& ec, std::size_t size) {
        if (!ec && size > 0 && stderrCallback) {
            std::string data(errBuffer.data(), size);
            stderrCallback(data);
            readErrors();
        } else if (ec == boost::asio::error::eof) {
            checkCompletion();
        }
    });
}

void AsyncProcess::checkCompletion() const {
    if (!childProcess.running() && endCallback) {
        endCallback(childProcess.exit_code());
    }
}

AsyncProcess::AsyncProcess(const std::string& command, const std::string& cwd):
    work(boost::asio::make_work_guard(ioContext)),
    outPipe(ioContext),
    errPipe(ioContext)
{
    boost::process::environment env = boost::this_process::environment();
    childProcess = boost::process::child(
        command,
        boost::process::std_in < in,
        boost::process::std_out > outPipe,
        boost::process::std_err > errPipe,
        ioContext,
        boost::process::start_dir(cwd.empty() ? boost::filesystem::current_path() : cwd),
        env
    );

    ioThread = std::thread([this]() { ioContext.run(); });

    readOutput();
    readErrors();
}

AsyncProcess::~AsyncProcess() {
    terminate();
    work.reset();
    if (ioThread.joinable()) {
        ioThread.join();
    }
}

void AsyncProcess::onStdout(const std::function<void(const std::string&)>& callback) {
    stdoutCallback = callback;
}

void AsyncProcess::onStderr(const std::function<void(const std::string&)>& callback) {
    stdoutCallback = callback;
}

void AsyncProcess::onEnd(const std::function<void(int)>& callback) {
    endCallback = callback;
}

void AsyncProcess::terminate() const {
    if (childProcess.running()) {
        childProcess.terminate();
    }
}

bool AsyncProcess::running() const {
    return childProcess.running();
}