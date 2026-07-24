import Foundation

extension WorkbenchModel {
    var canAmendCurrentRun: Bool { activeCampaign != nil && !isRunning }

    func amendCurrentRunFromPrompt() {
        guard let campaign = activeCampaign,
              !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        let request = prompt
        let resumeTime = virtualTimeNS
        let interventionTime = resumeTime.saturatingAdd(100_000_000)
        let selectedProvider = provider
        let wasPlaying = isPlaying
        let renderedState: LiveRunContext
        do {
            renderedState = try LiveRunContext.make(
                state: state,
                events: events,
                cursor: cursor,
                timeNS: resumeTime,
                campaign: campaign
            )
        } catch {
            errorMessage = error.localizedDescription
            status = "Could not capture the current experiment state."
            return
        }
        isRunning = true
        errorMessage = nil
        status = "Interpreting a change to the current experiment…"
        runTask = Task {
            do {
                let (amended, actualProvider) = try await PromptAuthor().amend(
                    prompt: request,
                    provider: selectedProvider,
                    campaign: campaign,
                    renderedState: renderedState,
                    at: interventionTime
                )
                guard !Task.isCancelled else { return }
                let normalized = try annotated(amended, author: actualProvider.rawValue)
                generatedSpec = String(decoding: normalized, as: UTF8.self)
                startRun(
                    campaign: normalized,
                    author: actualProvider.rawValue,
                    authoringPrompt: request,
                    resumeAtNS: resumeTime,
                    authoringContext: renderedState.data
                )
            } catch {
                isRunning = false
                isPlaying = wasPlaying
                errorMessage = error.localizedDescription
                status = "The current experiment was not changed."
            }
        }
    }
}
