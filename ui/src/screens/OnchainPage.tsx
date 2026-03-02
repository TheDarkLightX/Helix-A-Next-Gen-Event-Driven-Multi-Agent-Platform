import { FormEvent, useState } from "react";
import {
  OnchainBroadcastResponse,
  fetchOnchainReceipt,
  sendRawOnchainTransaction,
} from "../lib/api";

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

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero">
        <p className="mono-label">Onchain Shell</p>
        <h2>EVM Raw Transaction Submit + Receipt Polling</h2>
        <p>
          Deterministic transaction intent in core, imperative JSON-RPC execution in shell. Use
          dry-run mode first, then submit signed raw tx when ready.
        </p>
      </article>

      <article className="panel">
        <p className="mono-label">Broadcast Transaction</p>
        <form className="form-grid" onSubmit={onSubmit}>
          <label className="field field-full">
            <span>rpc_url</span>
            <input value={rpcUrl} onChange={(e) => setRpcUrl(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>raw_tx_hex</span>
            <textarea
              className="command-editor"
              rows={4}
              value={rawTxHex}
              onChange={(e) => setRawTxHex(e.target.value)}
            />
          </label>

          <label className="field">
            <span>max_poll_rounds</span>
            <input
              type="number"
              min={1}
              value={maxPollRounds}
              onChange={(e) => setMaxPollRounds(Number(e.target.value))}
            />
          </label>

          <label className="field">
            <span>poll_interval_ms</span>
            <input
              type="number"
              min={50}
              value={pollIntervalMs}
              onChange={(e) => setPollIntervalMs(Number(e.target.value))}
            />
          </label>

          <label className="field checkbox-field">
            <input
              type="checkbox"
              checked={awaitReceipt}
              onChange={(e) => setAwaitReceipt(e.target.checked)}
            />
            <span>await_receipt</span>
          </label>

          <label className="field checkbox-field">
            <input type="checkbox" checked={dryRun} onChange={(e) => setDryRun(e.target.checked)} />
            <span>dry_run</span>
          </label>

          <button className="btn-primary" type="submit">
            Execute Broadcast
          </button>
        </form>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel">
        <p className="mono-label">Result</p>
        {result ? (
          <div className="command-stack">
            <code className="command-inline">phase: {result.phase}</code>
            <code className="command-inline">tx_hash: {result.tx_hash ?? "none"}</code>
            <code className="command-inline">
              poll_rounds: {result.poll_rounds}/{result.max_poll_rounds}
            </code>
            <code className="command-inline">
              receipt_status: {result.receipt?.status ?? "none"}
            </code>
            <code className="command-inline">
              receipt_block: {result.receipt?.blockNumber ?? "none"}
            </code>
          </div>
        ) : (
          <p>No result yet.</p>
        )}
      </article>

      <article className="panel">
        <p className="mono-label">Manual Receipt Lookup</p>
        <form onSubmit={onLookupReceipt} className="form-grid">
          <label className="field field-full">
            <span>tx_hash</span>
            <input value={receiptHash} onChange={(e) => setReceiptHash(e.target.value)} />
          </label>
          <button className="btn-secondary" type="submit">
            Fetch Receipt
          </button>
        </form>
        <p className="status-line">{receiptStatus}</p>
      </article>
    </section>
  );
}
