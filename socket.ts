import fs from 'fs';

interface Socket {
    write(data: Uint8Array): void;
    end(): void;
    data: Connection;
    ended?: boolean;
}

class Connection {
    socket: Socket | null;
    connected: boolean;
    buffer: Uint8Array;
    dataCallback: ((data: Uint8Array) => void) | null;
    closeCallback: (() => void) | null;
    connectedCallback: (() => void) | null;
    data: any; // user data

    constructor(socket: Socket | null, isConnected: boolean = true) {
        this.socket = socket;
        this.connected = isConnected;
        this.buffer = new Uint8Array(0);
        this.dataCallback = null;
        this.closeCallback = null;
        this.connectedCallback = null;
        this.data = null;
        if (socket) {
            socket.data = this;
        }
    }

    _appendToBuffer(newData: Uint8Array): void {
        const combined = new Uint8Array(this.buffer.length + newData.length);
        combined.set(this.buffer);
        combined.set(newData, this.buffer.length);
        this.buffer = combined;
    }

    _processBuffer(): void {
        while (this.buffer.length >= 4) {
            const dv = new DataView(this.buffer.buffer, this.buffer.byteOffset, this.buffer.length);
            const len = dv.getUint32(0, true);
            if (this.buffer.length < 4 + len) break;
            const msg = this.buffer.subarray(4, 4 + len);
            if (this.dataCallback) this.dataCallback(msg);
            this.buffer = this.buffer.subarray(4 + len);
        }
    }

    write(data: string | Uint8Array): void {
        if (!this.socket) return;
        if (typeof data === 'string') data = new TextEncoder().encode(data);
        const lenBuf = new ArrayBuffer(4);
        new DataView(lenBuf).setUint32(0, data.length, true);
        const lenBytes = new Uint8Array(lenBuf);
        this.socket.write(new Uint8Array([...lenBytes, ...data]));
    }

    close(): void {
        if (this.socket) this.socket.end();
    }

    onData(cb: (data: Uint8Array) => void): void {
        this.dataCallback = cb;
    }

    onClose(cb: () => void): void {
        this.closeCallback = cb;
    }

    onConnected(cb: () => void): void {
        this.connectedCallback = cb;
        if (this.connected && cb) cb();
    }
}

interface BunServer {
    stop(): void;
}

class Server {
    private path: string;
    private connectCallback: ((conn: Connection) => void) | null;
    private server: BunServer | null;

    constructor(path: string) {
        this.path = path;
        this.connectCallback = null;
        this.server = null;
    }

    onConnect(cb: (conn: Connection) => void): void {
        this.connectCallback = cb;
    }

    start(startCallback?: () => void): void {
        this.server = Bun.listen({
            unix: this.path,
            socket: {
                open: (socket: Socket) => {
                    const conn = new Connection(socket);
                    if (this.connectCallback) this.connectCallback(conn);
                },
                data: (socket: Socket, buffer: Uint8Array) => {
                    const conn = socket.data;
                    conn._appendToBuffer(buffer);
                    conn._processBuffer();
                    if (conn.closeCallback && socket.ended) conn.closeCallback();
                },
                end: (socket: Socket) => {
                    const conn = socket.data;
                    if (conn.closeCallback) conn.closeCallback();
                },
            },
        });
        if (startCallback) startCallback();
    }

    stop(): void {
        if (this.server) this.server.stop();
    }
}

class Client {
    async connect(path: string): Promise<Connection> {
        const socket = await Bun.connect({
            unix: path,
            socket: {
                open: (socket: Socket) => {
                    const conn = socket.data;
                    conn.connected = true;
                    if (conn.connectedCallback) conn.connectedCallback();
                },
                data: (socket: Socket, buffer: Uint8Array) => {
                    const conn = socket.data;
                    conn._appendToBuffer(buffer);
                    conn._processBuffer();
                },
                end: (socket: Socket) => {
                    const conn = socket.data;
                    if (conn.closeCallback) conn.closeCallback();
                },
            },
        });
        const conn = new Connection(socket);
        socket.data = conn;
        return conn;
    }
}

export { Connection, Server, Client };