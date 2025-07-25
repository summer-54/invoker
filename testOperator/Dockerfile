FROM ubuntu:latest

#RUN apt update -y
#RUN apt install -y build-essential
#RUN apt install -y gcc g++ gdb
#RUN apt install -y libasan8 libubsan1 libtsan2
#RUN apt install -y valgrind cppcheck

WORKDIR /operator

COPY ./operator ./operator

CMD ["./operator"]