import { JsonRpcProvider, WebSocketProvider } from 'ethers'
import Config from './config'

export class EthProvider extends JsonRpcProvider {
    constructor() {
        const env = new Config()
        super(env.RPC_URL_HTTP)
    }
}

export class EthProviderWs extends WebSocketProvider {
    constructor() {
        const env = new Config()
        if (!env.RPC_URL_WS) throw new Error('RPC_URL_WS not set')
        super(env.RPC_URL_WS)
    }
}
