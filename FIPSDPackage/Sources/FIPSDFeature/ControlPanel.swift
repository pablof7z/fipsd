import SwiftUI

struct ControlPanel: View {
    @Bindable var model: WorkbenchModel
    let openArtifact: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Button("Open saved artifact", systemImage: "folder") { openArtifact() }
                    .accessibilityIdentifier("openArtifactButton")
                Divider()
                manualConfiguration
                Divider()
                fidelity
            }
            .padding(16)
        }
    }

    private var manualConfiguration: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Direct controls", systemImage: "slider.horizontal.3")
                    .font(.headline)
                Spacer()
                Button("10K preset") { model.loadTenThousandPreset() }
                    .controlSize(.small)
                Button("1B cohorts") { model.runBillionNodeCohort() }
                    .controlSize(.small)
                    .accessibilityIdentifier("billionCohortButton")
            }
            Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 8) {
                intRow("Nodes", value: $model.configuration.nodes)
                intRow("Lower roots", value: $model.configuration.arrivals)
                decimalRow("Arrival interval", value: $model.configuration.intervalSeconds, suffix: "s")
                decimalRow("Link latency", value: $model.configuration.latencyMilliseconds, suffix: "ms")
                decimalRow("Link jitter", value: $model.configuration.jitterMilliseconds, suffix: "ms")
                intRow("Loss", value: $model.configuration.lossPPM, suffix: "ppm")
                intRow("MTU", value: $model.configuration.mtuBytes, suffix: "B")
                decimalRow("Debounce", value: $model.configuration.debounceMilliseconds, suffix: "ms")
            }
            Picker("Topology", selection: $model.configuration.topology) {
                Text("Random regular").tag("random-regular")
                Text("Scale free").tag("scale-free")
                Text("Balanced tree").tag("balanced-tree")
                Text("Chain").tag("chain")
            }
            Picker("Arrival attachment", selection: $model.configuration.attachment) {
                Text("Different seeded points").tag("random")
                Text("Current root").tag("current-root")
                Text("Leaf").tag("leaf")
                Text("Hub").tag("hub")
                Text("Articulation point").tag("articulation")
            }
            TopologyEditorControls(model: model)
            MediaZoneControls(model: model)
            DisclosureGroup("Per-node connectivity") {
                VStack(alignment: .leading, spacing: 9) {
                    Toggle("Randomize node profiles", isOn: $model.configuration.mixedTransports)
                    Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 7) {
                        GridRow {
                            Text("Profile").foregroundStyle(.secondary)
                            Text("Weight").foregroundStyle(.secondary)
                            Text("Mbit/s").foregroundStyle(.secondary)
                        }
                        transportRow(
                            "Wi‑Fi",
                            weight: $model.configuration.wifiWeight,
                            mbps: $model.configuration.wifiMbps
                        )
                        transportRow(
                            "Bluetooth",
                            weight: $model.configuration.bluetoothWeight,
                            mbps: $model.configuration.bluetoothMbps
                        )
                        transportRow(
                            "Tor",
                            weight: $model.configuration.torWeight,
                            mbps: $model.configuration.torMbps
                        )
                        transportRow(
                            "Ethernet",
                            weight: $model.configuration.ethernetWeight,
                            mbps: $model.configuration.ethernetMbps
                        )
                    }
                    .disabled(!model.configuration.mixedTransports)
                    Text("Weights are deterministically sampled from the campaign seed. Profile values are modeled, not measured.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 6)
            }
            TrafficControls(model: model)
            ResourceControls(model: model)
            DisclosureGroup("Bloom propagation") {
                VStack(alignment: .leading, spacing: 8) {
                    Toggle(
                        "Animate split-horizon replacements",
                        isOn: $model.configuration.bloomEnabled
                    )
                    Text("Each peer replacement carries exact or declared-fidelity Bloom state over the real edge, sharing MTU, bandwidth, loss, and queues with tree and payload frames.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 6)
            }
            DisclosureGroup("Lookup & session recovery") {
                VStack(alignment: .leading, spacing: 8) {
                    Toggle(
                        "Resolve coordinates before payloads",
                        isOn: $model.configuration.lookupRecoveryEnabled
                    )
                    .disabled(!model.configuration.trafficEnabled)
                    intRow(
                        "Storm probes",
                        value: $model.configuration.lookupStormCount
                    )
                    Text("Cache misses animate exact-size lookup requests and reverse responses. Session setup and acknowledgement then traverse the same mixed-profile path before payload delivery.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 6)
            }
            DisclosureGroup("Inject event at cursor") {
                VStack(alignment: .leading, spacing: 9) {
                    Button("Add a lower root", systemImage: "arrow.down.to.line") {
                        model.scheduleLowerRoot()
                    }
                    Button("Rekey active sessions", systemImage: "key.horizontal") {
                        model.scheduleSessionRekey()
                    }
                    .disabled(
                        !model.configuration.trafficEnabled
                            || !model.configuration.lookupRecoveryEnabled
                            || !model.configuration.mixedTransports
                    )
                    Button("Expire caches + lookup storm", systemImage: "wave.3.right") {
                        model.scheduleLookupStorm()
                    }
                    .disabled(
                        !model.configuration.trafficEnabled
                            || !model.configuration.lookupRecoveryEnabled
                            || !model.configuration.mixedTransports
                    )
                    Picker(
                        "Transport class",
                        selection: $model.configuration.interventionTransportProfile
                    ) {
                        Text("Wi‑Fi").tag("wifi")
                        Text("Bluetooth").tag("bluetooth")
                        Text("Tor").tag("tor")
                        Text("Ethernet").tag("ethernet")
                    }
                    Button("Fail transport class", systemImage: "network.slash") {
                        model.scheduleTransportClass(restore: false)
                    }
                    .disabled(!model.configuration.mixedTransports)
                    Button("Restore transport class", systemImage: "network") {
                        model.scheduleTransportClass(restore: true)
                    }
                    .disabled(!model.configuration.mixedTransports)
                    ParentInterventionControls(model: model)
                    SybilInterventionControls(model: model)
                    Text("The event is scheduled 100 ms after the cursor and the campaign is replayed from virtual time zero.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    Divider()
                    Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 7) {
                        intRow("Edge", value: $model.interventionEdgeID)
                        intRow("Bandwidth", value: $model.interventionBandwidthMbps, suffix: "Mbit/s")
                        decimalRow(
                            "Latency",
                            value: $model.interventionLatencyMilliseconds,
                            suffix: "ms"
                        )
                        decimalRow(
                            "Jitter",
                            value: $model.interventionJitterMilliseconds,
                            suffix: "ms"
                        )
                        intRow("Loss", value: $model.interventionLossPPM, suffix: "ppm")
                        intRow("MTU", value: $model.interventionMTUBytes, suffix: "B")
                    }
                    HStack {
                        Button("Apply link", systemImage: "gauge.with.dots.needle.50percent") {
                            model.scheduleLinkChange(restore: false)
                        }
                        Button("Restore", systemImage: "arrow.uturn.backward") {
                            model.scheduleLinkChange(restore: true)
                        }
                    }
                    .controlSize(.small)
                }
                .padding(.top, 6)
            }
            VariantComparisonControls(model: model)
            SearchControls(model: model)
            TinyExplorerControls(model: model)
            Button("Run configured scenario", systemImage: "play.circle.fill") { model.runConfigured() }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("runConfiguredButton")
                .disabled(model.isRunning)
        }
    }

    private var fidelity: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label("Fidelity contract", systemImage: "checkmark.shield")
                .font(.headline)
            Text(model.summary.fidelity)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text("Changing controls starts a new deterministic run. It never mutates history behind the timeline.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func intRow(_ title: String, value: Binding<Int>, suffix: String = "") -> some View {
        GridRow {
            Text(title).foregroundStyle(.secondary)
            TextField(title, value: value, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 90)
            Text(suffix).foregroundStyle(.tertiary)
        }
    }

    private func decimalRow(_ title: String, value: Binding<Double>, suffix: String) -> some View {
        GridRow {
            Text(title).foregroundStyle(.secondary)
            TextField(title, value: value, format: .number.precision(.fractionLength(0...3)))
                .textFieldStyle(.roundedBorder)
                .frame(width: 90)
            Text(suffix).foregroundStyle(.tertiary)
        }
    }

    private func transportRow(
        _ title: String,
        weight: Binding<Int>,
        mbps: Binding<Int>
    ) -> some View {
        GridRow {
            Text(title)
            TextField("Weight", value: weight, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 58)
            TextField("Bandwidth", value: mbps, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 72)
        }
    }
}
