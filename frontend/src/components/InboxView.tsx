import { Layers, ChevronRight, Check, X } from 'lucide-react';

export interface ClusterItem {
  id: string;
  suggestedTitle: string;
  confidence: number;
  fileCount: number;
}

interface InboxViewProps {
  clusters: ClusterItem[];
  onSelectCluster: (id: string) => void;
  onAcceptCluster?: (id: string) => void;
  onRejectCluster?: (id: string) => void;
}

export function InboxView({ clusters, onSelectCluster, onAcceptCluster, onRejectCluster }: InboxViewProps) {
  return (
    <div className="p-12">
      {/* Header */}
      <header className="mb-12">
        <h1 className="text-white tracking-tight">Inbox</h1>
        <p className="mt-2 text-[#6b6b6b]">
          {clusters.length} Pending {clusters.length === 1 ? 'Match' : 'Matches'}
        </p>
      </header>

      {/* Cluster List */}
      <div className="space-y-4 max-w-4xl">
        {clusters.map((cluster) => (
          <div
            key={cluster.id}
            className="group relative flex items-center gap-6 p-6 bg-[#1e1e1e] border border-[#2a2a2a] rounded-lg hover:border-[#3a3a3a] hover:bg-[#242424] transition-all"
          >
            {/* Icon */}
            <div className="flex-shrink-0 w-16 h-16 bg-[#2a2a2a] border border-[#3a3a3a] rounded flex items-center justify-center">
              <Layers className="w-8 h-8 text-[#7ba8c7]" strokeWidth={1.5} />
            </div>

            {/* Content - Clickable area */}
            <button
              onClick={() => onSelectCluster(cluster.id)}
              className="flex-1 text-left"
            >
              <h3 className="text-white mb-1">Suggested Match: {cluster.suggestedTitle}</h3>
              <p className="text-[#7ba8c7]">
                <small>Confidence: {cluster.confidence}% · {cluster.fileCount} files</small>
              </p>
            </button>

            {/* Hover Actions */}
            <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
              {onAcceptCluster && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onAcceptCluster(cluster.id);
                  }}
                  className="flex items-center gap-2 px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors"
                  title="Accept Match"
                >
                  <Check className="w-5 h-5" strokeWidth={2} />
                  <span><small>Accept</small></span>
                </button>
              )}
              
              {onRejectCluster && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onRejectCluster(cluster.id);
                  }}
                  className="flex items-center gap-2 px-4 py-2 bg-[#2a2a2a] text-red-400 border border-[#3a3a3a] rounded-lg hover:bg-red-900/20 hover:border-red-900 transition-colors"
                  title="Reject Match"
                >
                  <X className="w-5 h-5" strokeWidth={2} />
                  <span><small>Reject</small></span>
                </button>
              )}

              <ChevronRight 
                className="w-5 h-5 text-[#6b6b6b] ml-2" 
                strokeWidth={2}
              />
            </div>
          </div>
        ))}

        {clusters.length === 0 && (
          <div className="text-center py-20 text-[#6b6b6b]">
            <p>No pending matches</p>
          </div>
        )}
      </div>
    </div>
  );
}