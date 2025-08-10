import {Connection, Server} from './socket';
import WebSocketClient from './websocketClient';
import Session from './session';
import PodmanClient from "./podmanClient.js";

export const podmanClient = new PodmanClient("unix:///run/user/1000/podman/podman.sock");

const SOCKET_PATH = '/tmp/invoker.sock';
export const client = new WebSocketClient('ws://localhost:9000/invoker');
client.connect();
const server = new Server(SOCKET_PATH);

server.onConnect((conn: Connection) => {
    conn.onData((data: Uint8Array) => {
        const str = new TextDecoder().decode(data);
        const session = conn.data as Session | null;
        if (session) {
            session.onData(str);
            return;
        }
        for (const task of client.tasks_.values()) {
            if (task.getToken() === str) {
                const session = new Session(task.getNetworks(), task.getVolumePath(), conn, task);
                conn.data = session;
                task.session = session;
                break;
            }
        }
    });
    conn.onClose(() => {
        console.log('Connection closed');
    });
});

server.start(() => {
    console.log('started');
});