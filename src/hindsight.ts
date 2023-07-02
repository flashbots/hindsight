import Matchmaker from '@flashbots/matchmaker-ts'
import { Wallet } from 'ethers'
import Config from './config'
import { EthProvider, EthProviderWs } from './provider'

export default class Hindsight {
    private matchmaker?: Matchmaker
    private readonly authSigner: Wallet
    private readonly provider: EthProvider | EthProviderWs

    /**
     * @constructor Create a new Hindsight instance, connect providers
     */
    constructor() {
        console.log('hindsight')
        const env = new Config()
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
        const eventInfo = await this.matchmaker.getEventHistoryInfo()
        const latestBlock = await this.provider.getBlockNumber()

        return {}
    }
}
