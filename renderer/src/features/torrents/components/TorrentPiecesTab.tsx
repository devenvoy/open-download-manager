import { formatBytes } from "@/lib/format";
import { type TorrentPiecesInfo } from "@/types/ipc";

export interface TorrentPiecesTabProps {
  piecesInfo?: TorrentPiecesInfo;
  loading: boolean;
}

const MAX_GRID_CELLS = 600;

export function TorrentPiecesTab({ piecesInfo, loading }: TorrentPiecesTabProps) {
  if (loading && !piecesInfo) {
    return (
      <div id="torrent-tab-pieces" className="torrent-tab-panel" role="tabpanel">
        <p className="torrent-tab-empty">Loading swarm piece map…</p>
      </div>
    );
  }

  if (!piecesInfo || piecesInfo.totalPieces === 0) {
    return (
      <div id="torrent-tab-pieces" className="torrent-tab-panel" role="tabpanel">
        <p className="torrent-tab-empty">No piece metadata available for this torrent.</p>
      </div>
    );
  }

  const { totalPieces, pieceLength, completedPieces, pieceStates } = piecesInfo;
  const pct = totalPieces > 0 ? ((completedPieces / totalPieces) * 100).toFixed(1) : "0.0";

  // Downsample to MAX_GRID_CELLS if total pieces exceeds limit
  const displayCells: number[] = [];
  if (pieceStates.length <= MAX_GRID_CELLS) {
    for (const state of pieceStates) {
      displayCells.push(state);
    }
  } else {
    const bucketSize = pieceStates.length / MAX_GRID_CELLS;
    for (let i = 0; i < MAX_GRID_CELLS; i++) {
      const start = Math.floor(i * bucketSize);
      const end = Math.min(pieceStates.length, Math.floor((i + 1) * bucketSize));
      let hasDownloading = false;
      let haveCount = 0;
      for (let j = start; j < end; j++) {
        if (pieceStates[j] === 2) haveCount++;
        else if (pieceStates[j] === 1) hasDownloading = true;
      }
      if (haveCount === end - start) {
        displayCells.push(2); // Fully have
      } else if (hasDownloading) {
        displayCells.push(1); // In flight
      } else if (haveCount > 0) {
        displayCells.push(1); // Partial in bucket
      } else {
        displayCells.push(0); // Missing
      }
    }
  }

  return (
    <div id="torrent-tab-pieces" className="torrent-tab-panel" role="tabpanel">
      <div className="torrent-pieces-summary">
        <div className="piece-stat-item">
          <span className="stat-label">Pieces</span>
          <span className="stat-value">
            {completedPieces.toLocaleString()} / {totalPieces.toLocaleString()} ({pct}%)
          </span>
        </div>
        {pieceLength > 0 && (
          <div className="piece-stat-item">
            <span className="stat-label">Piece Size</span>
            <span className="stat-value">{formatBytes(pieceLength)}</span>
          </div>
        )}
      </div>

      <div className="piece-grid-container">
        <div className="piece-grid-matrix" role="img" aria-label="Torrent pieces matrix">
          {displayCells.map((state, idx) => {
            const stateClass =
              state === 2
                ? "piece-have"
                : state === 1
                ? "piece-downloading"
                : "piece-missing";
            return (
              <div
                key={idx}
                className={`piece-cell ${stateClass}`}
                title={`Piece ${idx + 1}: ${state === 2 ? "Completed" : state === 1 ? "In-flight" : "Missing"}`}
              />
            );
          })}
        </div>
      </div>

      <div className="piece-legend">
        <span className="legend-item">
          <span className="legend-swatch piece-have" /> Have ({completedPieces})
        </span>
        <span className="legend-item">
          <span className="legend-swatch piece-downloading" /> In-flight
        </span>
        <span className="legend-item">
          <span className="legend-swatch piece-missing" /> Missing (
          {(totalPieces - completedPieces).toLocaleString()})
        </span>
      </div>
    </div>
  );
}
