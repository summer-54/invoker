FROM ubuntu:latest as ubuntu-libsan

#RUN apt update -y
#RUN apt install -y build-essential
#RUN apt install -y gcc g++ gdb
#RUN apt install -y libasan8 libubsan1 libtsan2
#RUN apt install -y valgrind cppcheck

FROM ubuntu-libsan

WORKDIR /operator

COPY ./operator ./operator

CMD ["./operator"]