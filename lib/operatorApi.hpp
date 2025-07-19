#pragma once
#include "lib/socket.hpp"
#include <functional>
#include <vector>
#include <string>
#include <map>
#include <sstream>

class OperatorApi {
public:
    enum class STDOUT {none, onEnd, normal};

    enum class Verdict {OK, WA, TL, ML, ITL, RTL, RML, CE, ERR};

protected:
    Socket::Client* client;
    Socket::Connection* connection;
    int imagesCount = 0, containersCount = 0;
    std::vector<std::function<void(const std::string&)>> callbacks;

    static std::string stringValue(STDOUT value);
    static std::string stringValue(Verdict value);

    class Container;

    class ContainerTemplate {
    protected:
        int image;
        OperatorApi* operatorApi;

    public:
        STDOUT stdout = STDOUT::normal, stderr = STDOUT::onEnd;
        std::vector<int> ports;
        std::vector<std::pair<std::string, std::string>> volumes;
        std::map<std::string, std::string> env;
        std::string initStdin;

        explicit ContainerTemplate(int image, OperatorApi* operatorApi);

        void onStdout(const std::function<void(const std::string&)>& callback) const;
        void onStderr(const std::function<void(const std::string&)>& callback) const;

        template<typename Type>
        Container& operator<<(const Type& chunk) {
            std::ostringstream stream;
            stream << chunk;
            initStdin += stream.str();
        }

        Container* run();
    };

    class Container {
    protected:
        int id;
        ContainerTemplate* containerTemplate;
        OperatorApi* operatorApi;

    public:
        explicit Container(int id, ContainerTemplate* containerTemplate, OperatorApi* operatorApi);

        void onStdout(const std::function<void(const std::string&)>& callback) const;
        void onStderr(const std::function<void(const std::string&)>& callback) const;

        void restart() const;

        void stop() const;

        void write(const std::string& chunk) const;

        template<typename Type>
        Container& operator<<(const Type& chunk) {
            std::ostringstream stream;
            stream << chunk;
            write(stream.str());
        }

        void getPort(int port, const std::function<void(int)>& callback) const;
    };

public:
    explicit OperatorApi(const std::string& socket);

    std::function<ContainerTemplate*()> build(const std::string& context, const std::string& dockerfilePath);

    void setVerdict(const std::string& subtaskId, Verdict verdict, const std::string& data) const;
    void setVerdict(Verdict verdict, const std::string& data) const;
};