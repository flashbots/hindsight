// parse cli params and env vars
// cli params take precedence over env vars if both exist
// TODO: implement CLI params w/ oclif
import { config } from 'dotenv'

export interface Config {
    readonly RPC_URL_HTTP: string
    readonly RPC_URL_WS?: string
    readonly AUTH_SIGNER_PRIVATE_KEY: string
}

export class Env implements Config {
    readonly RPC_URL_HTTP: string
    readonly RPC_URL_WS?: string | undefined
    readonly AUTH_SIGNER_PRIVATE_KEY: string
    constructor() {
        config()
        if (!process.env.RPC_URL_HTTP) throw new Error('RPC_URL_HTTP not set')
        if (!process.env.AUTH_SIGNER_PRIVATE_KEY) throw new Error('AUTH_SIGNER_PRIVATE_KEY not set')

        this.RPC_URL_HTTP = process.env.RPC_URL_HTTP
        this.RPC_URL_WS = process.env.RPC_URL_WS
        this.AUTH_SIGNER_PRIVATE_KEY = process.env.AUTH_SIGNER_PRIVATE_KEY
    }
}
