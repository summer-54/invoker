import ws from 'ws';
import fs from 'fs';
import Task from './task';

class WebSocketClient {
    private uri: string;
    private ws: ws | null;
    private connected: boolean;
    tasks_: Map<string, Task>;

    constructor(uri: string) {
        this.uri = uri;
        this.ws = null;
        this.connected = false;
        this.tasks_ = new Map();
    }

    connect(): void {
        this.ws = new ws(this.uri);
        this.ws.on('open', () => {
            this.connected = true;
            console.log('Connected to ' + this.uri);
        });
        this.ws.on('close', () => {
            this.connected = false;
            console.log('WebSocket connection closed');
        });
        this.ws.on('message', (data: Buffer) => this.onMessage(data));
        this.ws.on('error', (err: Error) => {
            console.log('WebSocket error: ' + err.message);
            this.connected = false;
        });
    }

    disconnect(): void {
        if (this.ws) this.ws.close();
    }

    isConnected(): boolean {
        return this.connected;
    }

    sendWebSocketMessage(message: string): boolean {
        if (!this.connected) {
            console.error('Not connected to server');
            return false;
        }
        this.ws!.send(message);
        return true;
    }

    static formatFullVerdictMessage(taskId: string, verdict: string, data: string): string {
        return `${taskId}\nVERDICT ${verdict}\n${data}`;
    }

    static formatSubtaskVerdictMessage(taskId: string, subtaskId: string, verdict: string, data: string): string {
        return `${taskId}\nSUBTASK ${subtaskId}\nVERDICT ${verdict}\n${data}`;
    }

    static formatExitedMessage(taskId: string, exitCode: number, exitData: string): string {
        return `${taskId}\nEXITED ${exitCode}\n${exitData}`;
    }

    static formatErrorMessage(taskId: string, errorType: string, errorMessage: string): string {
        return `${taskId}\n${errorType}\n${errorMessage}`;
    }

    sendFullVerdict(taskId: string, verdict: string, data: string): boolean {
        return this.sendWebSocketMessage(WebSocketClient.formatFullVerdictMessage(taskId, verdict, data));
    }

    sendSubtaskVerdict(taskId: string, subtaskId: string, verdict: string, data: string): boolean {
        return this.sendWebSocketMessage(WebSocketClient.formatSubtaskVerdictMessage(taskId, subtaskId, verdict, data));
    }

    sendExited(taskId: string, exitCode: number, exitData: string): boolean {
        return this.sendWebSocketMessage(WebSocketClient.formatExitedMessage(taskId, exitCode, exitData));
    }

    sendInvokerError(taskId: string, errorMessage: string): boolean {
        return this.sendWebSocketMessage(WebSocketClient.formatErrorMessage(taskId, 'ERROR', errorMessage));
    }

    sendOperatorError(taskId: string, errorMessage: string): boolean {
        return this.sendWebSocketMessage(WebSocketClient.formatErrorMessage(taskId, 'OPERROR', errorMessage));
    }

    async onMessage(data: Buffer): Promise<void> {
        console.log('WebSocket message received');
        const nlPos = Array.from(data).findIndex((byte) => byte === 10);
        if (nlPos === -1) {
            console.error('Invalid message: no header found');
            return;
        }
        const header = data.slice(0, nlPos).toString('utf-8');
        const parts = header.split(' ');
        if (parts.length < 2) return;
        const taskId = parts[0];
        const type = parts[1];
        if (!taskId || !type) return;
        if (type === 'START') {
            const tarData = data.slice(nlPos + 1);
            fs.writeFileSync('./test0.tar.gz', tarData);
            if (this.tasks_.has(taskId)) {
                console.error('Task exists ' + taskId);
                return;
            }
            const task = await Task.create(taskId, tarData);
            this.tasks_.set(taskId, task);
        }
    }
}

export default WebSocketClient;