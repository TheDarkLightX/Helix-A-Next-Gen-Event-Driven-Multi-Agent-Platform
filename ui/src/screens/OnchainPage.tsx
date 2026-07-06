import { FormEvent, useState } from "react";
import {
  OnchainBroadcastResponse,
  fetchOnchainReceipt,
  sendRawOnchainTransaction,
} from "../lib/api";
import {
  Panel,
  FormField,
  Input,
  Textarea,
  CheckboxField,
  Button,
  Badge,
  StatusLine,
} from "../components";

export function OnchainPage() {
  const [rpcUrl, setRpcUrl] = useState<string>("https://rpc.ankr.com/eth");
  const [rawTxHex, setRawTxHex] = useState<string>("0xdeadbeef");
  const [awaitReceipt, setAwaitReceipt] = useState<boolean>(true);
  const [dryRun, setDryRun] = useState<boolean>(true);
  const [maxPollRounds, setMaxPollRounds] = useState<number>(20);
  const [pollIntervalMs, setPollIntervalMs] = useState<number>(500);
  const [status, setStatus] = useState<string>("");
  const [result, setResult] = useState<OnchainBroadcastResponse | null>(null);

  const [receiptHash, setReceiptHash] = useState<string>("");
  const [receiptStatus, setReceiptStatus] = useState<string>("");

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setStatus("Submitting transaction intent...");
    setResult(null);
    try {
      const response = await sendRawOnchainTransaction({
        rpc_url: rpcUrl,
        raw_tx_hex: rawTxHex,
        await_receipt: awaitReceipt,
        max_poll_rounds: maxPollRounds,
        poll_interval_ms: pollIntervalMs,
        dry_run: dryRun,
      });
      setResult(response);
      setReceiptHash(response.tx_hash ?? "");
      setStatus(
        `Completed with phase=${response.phase}, polls=${response.poll_rounds}/${response.max_poll_rounds}`
      );
    } catch (error) {
      setStatus(`Submit failed: ${(error as Error).message}`);
    }
  }

  async function onLookupReceipt(event: FormEvent) {
    event.preventDefault();
    setReceiptStatus("Fetching receipt...");
    try {
      const response = await fetchOnchainReceipt(rpcUrl, receiptHash);
      if (!response.found) {
        setReceiptStatus("Receipt not found yet.");
      } else {
        setReceiptStatus(
          `Receipt found: status=${response.receipt?.status ?? "unknown"}, block=${response.receipt?.blockNumber ?? "unknown"}`
        );
      }
    } catch (error) {
      setReceiptStatus(`Lookup failed: ${(error as Error).message}`);
    }
  }

  const phaseTone = (phase: string) =>
    phase === "Confirmed" ? "ok" : phase === "Reverted" || phase === "Failed" ? "danger" : phase === "Idle" ? "neutral" : "info";

  return (
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Onchain Shell" title="EVM Raw Transaction Submit + Receipt Polling">
        <p className="hx-description">
          Deterministic transaction intent in core, imperative JSON-RPC execution in
          shell. Use dry-run mode first, then submit signed raw tx when ready.
        </p>
      </Panel>

      <Panel span={6} eyebrow="Broadcast" title="Transaction Intent">
        <form className="hx-form-grid" onSubmit={onSubmit}>
          <FormField label="RPC URL" full>
            <Input value={rpcUrl} onChange={(e) => setRpcUrl(e.target.value)} />
          </FormField>

          <FormField label="Raw TX Hex" full>
            <Textarea rows={4} value={rawTxHex} onChange={(e) => setRawTxHex(e.target.value)} />
          </FormField>

          <FormField label="Max poll rounds">
            <Input type="number" min={1} value={maxPollRounds} onChange={(e) => setMaxPollRounds(Number(e.target.value))} />
          </FormField>

          <FormField label="Poll interval (ms)">
            <Input type="number" min={50} value={pollIntervalMs} onChange={(e) => setPollIntervalMs(Number(e.target.value))} />
          </FormField>

          <CheckboxField label="Await receipt" checked={awaitReceipt} onChange={setAwaitReceipt} />
          <CheckboxField label="Dry run" checked={dryRun} onChange={setDryRun} />

          <FormField label="" full>
            <Button type="submit" variant={dryRun ? "primary" : "danger"}>
              {dryRun ? "Dry Run Broadcast" : "Execute Broadcast"}
            </Button>
          </FormField>
        </form>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={3} eyebrow="Result" title="Broadcast Output">
        {result ? (
          <div className="hx-list">
            <div className="hx-row">
              <span className="hx-row-primary">phase</span>
              <Badge tone={phaseTone(result.phase)}>{result.phase}</Badge>
            </div>
            <div className="hx-row hx-row-stack">
              <span className="hx-row-primary">tx_hash</span>
              <code className="hx-mono-detail">{result.tx_hash ?? "none"}</code>
            </div>
            <div className="hx-row">
              <span className="hx-row-primary">poll rounds</span>
              <span className="hx-row-secondary">{result.poll_rounds}/{result.max_poll_rounds}</span>
            </div>
            <div className="hx-row">
              <span className="hx-row-primary">receipt status</span>
              <span className="hx-row-secondary">{result.receipt?.status ?? "none"}</span>
            </div>
            <div className="hx-row">
              <span className="hx-row-primary">receipt block</span>
              <span className="hx-row-secondary">{result.receipt?.blockNumber ?? "none"}</span>
            </div>
          </div>
        ) : (
          <div className="hx-table-empty"><p>No result yet.</p></div>
        )}
      </Panel>

      <Panel span={3} eyebrow="Lookup" title="Manual Receipt">
        <form onSubmit={onLookupReceipt} className="hx-form-grid">
          <FormField label="TX Hash" full>
            <Input value={receiptHash} onChange={(e) => setReceiptHash(e.target.value)} placeholder="0x..." />
          </FormField>
          <FormField label="" full>
            <Button type="submit" variant="secondary">Fetch Receipt</Button>
          </FormField>
        </form>
        <StatusLine>{receiptStatus}</StatusLine>
      </Panel>
    </section>
  );
}
