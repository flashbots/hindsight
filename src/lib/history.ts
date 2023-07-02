import Matchmaker from '@flashbots/matchmaker-ts'

export async function getLatestMevShareTxs(matchmaker: Matchmaker) {
    return await matchmaker.getEventHistoryInfo()
}
