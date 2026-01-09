import { FolderOpen } from 'lucide-react';

interface OrphanCardProps {
  path: string;
  fileName: string;
}

export function OrphanCard({ path, fileName }: OrphanCardProps) {
  return (
    <div className="group cursor-pointer">
      <div className="relative aspect-[2/3] flex flex-col items-center justify-center bg-[#1a1a1a] border border-[#2a2a2a] rounded-sm p-8 transition-colors duration-300 hover:bg-[#1e1e1e] hover:border-[#3a3a3a]">
        {/* Icon */}
        <FolderOpen 
          className="w-16 h-16 text-[#4a4a4a] mb-6 transition-colors duration-300 group-hover:text-[#5a5a5a]" 
          strokeWidth={1}
        />
        
        {/* File name */}
        <div className="text-center w-full">
          <p className="text-[#b3b3b3] mb-4 break-all">{fileName}</p>
          
          {/* Path in monospace */}
          <code className="text-[#6b6b6b] block break-all text-center">
            {path}
          </code>
        </div>
        
        {/* Status indicator */}
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2">
          <small className="text-[#5a5a5a]">Unidentified</small>
        </div>
      </div>
      
      <div className="mt-4 px-1">
        <h3 className="text-[#6b6b6b]">Raw Data</h3>
        <p className="mt-1 text-[#4a4a4a]">
          <small>Awaiting Classification</small>
        </p>
      </div>
    </div>
  );
}
