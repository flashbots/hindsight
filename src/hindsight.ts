import Matchmaker from '@flashbots/matchmaker-ts'
import { Wallet } from 'ethers'
import { Env } from './env'
import { EthProvider, EthProviderWs } from './provider'
import { getLatestMevShareTxs } from './lib/history'

export default class Hindsight {
    private matchmaker?: Matchmaker
    private readonly authSigner: Wallet
    private readonly provider: EthProvider | EthProviderWs

    /**
     * @constructor Create a new Hindsight instance, connect providers
     */
    constructor() {
        console.log('hindsight')
        const env = new Env()
        this.authSigner = new Wallet(env.AUTH_SIGNER_PRIVATE_KEY)
        if (env.RPC_URL_WS) {
            this.provider = new EthProviderWs()
        } else {
            this.provider = new EthProvider()
        }
    }

    /**
     * @method init Connect to mev-share using authSigner and provider from constructor.
    */
    public async init(): Promise<Hindsight> {
        this.matchmaker = Matchmaker.fromNetwork(this.authSigner, await this.provider.getNetwork())
        return this
    }

    public async getMevShareHistory() {
        if (!this.matchmaker) {
            throw new Error('Matchmaker not initialized')
        }
        return getLatestMevShareTxs(this.matchmaker)
    }
}
