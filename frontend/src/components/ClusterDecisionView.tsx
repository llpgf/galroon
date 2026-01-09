import { FolderOpen, Check, X, ArrowLeft } from 'lucide-react';
import { ImageWithFallback } from './figma/ImageWithFallback';

export interface ClusterDecision {
  id: string;
  // File System Side
  detectedFolder: string;
  fileSize: string;
  fileCount: number;
  // Metadata Side
  suggestedTitle: string;
  suggestedCover?: string;
  developer?: string;
  releaseYear?: string;
  confidence: number;
}

interface ClusterDecisionViewProps {
  cluster: ClusterDecision;
  onBack: () => void;
  onAccept: () => void;
  onReject: () => void;
}

export function ClusterDecisionView({ cluster, onBack, onAccept, onReject }: ClusterDecisionViewProps) {
  return (
    <div className="min-h-screen flex flex-col">
      {/* Back Button */}
      <button
        onClick={onBack}
        className="fixed top-6 left-72 z-20 flex items-center gap-2 px-4 py-2 bg-[#1e1e1e]/90 backdrop-blur-sm border border-[#3a3a3a] rounded-lg text-[#b3b3b3] hover:text-white hover:border-[#4a4a4a] transition-all"
      >
        <ArrowLeft className="w-4 h-4" strokeWidth={2} />
        <span><small>Back to Inbox</small></span>
      </button>

      {/* Split View */}
      <div className="flex-1 grid grid-cols-2 gap-0 pt-20 pb-32">
        {/* Left Panel - File System (Reality) */}
        <div className="px-12 py-16 bg-[#1a1a1a] border-r border-[#2a2a2a] flex flex-col">
          <div className="max-w-lg mx-auto w-full">
            <h2 className="text-[#6b6b6b] mb-8">Detected Files</h2>
            
            <div className="space-y-8">
              {/* Folder Icon */}
              <div className="flex items-center justify-center py-12">
                <div className="w-32 h-32 bg-[#242424] border-2 border-dashed border-[#3a3a3a] rounded-lg flex items-center justify-center">
                  <FolderOpen className="w-16 h-16 text-[#4a4a4a]" strokeWidth={1} />
                </div>
              </div>

              {/* File Info */}
              <div className="space-y-4">
                <div>
                  <small className="text-[#6b6b6b] block mb-2">Folder Path</small>
                  <code className="text-[#b3b3b3] block bg-[#1e1e1e] p-4 rounded border border-[#2a2a2a] break-all">
                    {cluster.detectedFolder}
                  </code>
                </div>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <small className="text-[#6b6b6b] block mb-2">File Count</small>
                    <p className="text-[#b3b3b3]">{cluster.fileCount} files</p>
                  </div>
                  <div>
                    <small className="text-[#6b6b6b] block mb-2">Total Size</small>
                    <p className="text-[#b3b3b3]">{cluster.fileSize}</p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Right Panel - Metadata (Suggestion) */}
        <div className="px-12 py-16 bg-[#121212] flex flex-col">
          <div className="max-w-lg mx-auto w-full">
            <h2 className="text-white mb-8">Suggested Match</h2>
            
            <div className="space-y-8">
              {/* Cover Art */}
              {cluster.suggestedCover && (
                <div className="w-64 aspect-[2/3] rounded-lg overflow-hidden shadow-2xl mx-auto">
                  <ImageWithFallback
                    src={cluster.suggestedCover}
                    alt={cluster.suggestedTitle}
                    className="w-full h-full object-cover"
                  />
                </div>
              )}

              {/* Metadata */}
              <div className="space-y-4">
                <div>
                  <small className="text-[#6b6b6b] block mb-2">Title</small>
                  <h3 className="text-white text-2xl">{cluster.suggestedTitle}</h3>
                </div>

                {(cluster.developer || cluster.releaseYear) && (
                  <div>
                    <small className="text-[#6b6b6b] block mb-2">Details</small>
                    <p className="text-[#b3b3b3]">
                      {[cluster.developer, cluster.releaseYear].filter(Boolean).join(' · ')}
                    </p>
                  </div>
                )}

                <div>
                  <small className="text-[#6b6b6b] block mb-2">Confidence</small>
                  <div className="flex items-center gap-3">
                    <div className="flex-1 h-2 bg-[#2a2a2a] rounded-full overflow-hidden">
                      <div
                        className="h-full bg-[#7ba8c7]"
                        style={{ width: `${cluster.confidence}%` }}
                      />
                    </div>
                    <span className="text-[#7ba8c7]">{cluster.confidence}%</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Sticky Footer - Decision Buttons */}
      <div className="fixed bottom-0 left-60 right-0 bg-[#1e1e1e] border-t border-[#2a2a2a] px-12 py-6">
        <div className="flex items-center justify-between max-w-6xl mx-auto">
          <button
            onClick={onReject}
            className="flex items-center gap-2 px-6 py-3 text-[#6b6b6b] hover:text-red-400 transition-colors"
          >
            <X className="w-5 h-5" strokeWidth={2} />
            <span>Reject</span>
          </button>

          <button
            onClick={onAccept}
            className="flex items-center gap-2 px-8 py-3 bg-white text-black rounded-lg hover:bg-[#f0f0f0] transition-colors"
          >
            <Check className="w-5 h-5" strokeWidth={2} />
            <span>Accept Match</span>
          </button>
        </div>
      </div>
    </div>
  );
}
