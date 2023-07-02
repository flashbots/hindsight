import Matchmaker, {EventHistoryEntry} from '@flashbots/matchmaker-ts'
import { Wallet } from 'ethers'
import Config from './config'
import { EthProvider, EthProviderWs } from './provider'

export class Hindsight {
    public readonly matchmaker: Matchmaker
    public readonly authSigner: Wallet
    public readonly provider: EthProvider | EthProviderWs

    constructor(matchmaker: Matchmaker, authSigner: Wallet, provider: EthProvider | EthProviderWs) {
        this.matchmaker = matchmaker
        this.authSigner = authSigner
        this.provider = provider
    }

    public async getMevShareHistory() {
        if (!this.matchmaker) {
            throw new Error('Matchmaker not initialized')
        }
        const eventInfo = await this.matchmaker.getEventHistoryInfo()
        const latestBlock = await this.provider.getBlockNumber()
        console.log("latest block", latestBlock)

        let done = false
        let i = 0
        let events: Array<EventHistoryEntry> = []
        while (!done) {
            const mevShareHistory = await this.matchmaker.getEventHistory({
                blockStart: latestBlock - 256,
                limit: eventInfo.maxLimit,
                offset: i * eventInfo.maxLimit,
            })
            i++
            if (mevShareHistory.length < eventInfo.maxLimit) {
                done = true
            }
            events.push(...mevShareHistory)
            console.log(`accumulated ${events.length} events`)
        }
        return events
    }

    public destroy() {
        return this.provider.destroy()
    }
}

export default class HindsightFactory {
    public matchmaker?: Matchmaker
    public readonly authSigner: Wallet
    public readonly provider: EthProvider | EthProviderWs

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
     * @method init Connect to mev-share using authSigner and provider from constructor. Return a new Hindsight instance.
    */
    public async init(): Promise<Hindsight> {
        this.matchmaker = Matchmaker.fromNetwork(this.authSigner, await this.provider.getNetwork())
        return new Hindsight(this.matchmaker, this.authSigner, this.provider)
    }
}
