import { JsonRpcProvider, WebSocketProvider } from 'ethers'
import { Env } from './env'

export class EthProvider extends JsonRpcProvider {
    constructor() {
        const env = new Env()
        super(env.RPC_URL_HTTP)
    }
}

export class EthProviderWs extends WebSocketProvider {
    constructor() {
        const env = new Env()
        if (!env.RPC_URL_WS) throw new Error('RPC_URL_WS not set')
        super(env.RPC_URL_WS)
    }
}
