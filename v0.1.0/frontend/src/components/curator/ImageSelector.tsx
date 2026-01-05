/**
 * ImageSelector - Roon-style Image Curator
 *
 * Phase 24.0: The Curator
 *
 * Features:
 * - Gallery view of all images in game folder
 * - Set as Cover / Set as Background actions
 * - Download from VNDB
 * - Visual feedback (green border/check badge)
 */

import React, { useState, useEffect } from 'react';
import { Check, Download, X, Image as ImageIcon, Star } from 'lucide-react';
import { api } from '../../api/client';

interface GameImage {
  path: string;
  name: string;
  type: 'cover' | 'background' | 'screenshot' | 'other';
  url: string;
}

interface ImageSelectorProps {
  gameId: string;
  gamePath: string;
  currentCover?: string;
  currentBackground?: string;
  onCoverSelected: (imagePath: string) => void;
  onBackgroundSelected: (imagePath: string) => void;
  onClose: () => void;
}

type SelectionType = 'cover' | 'background' | null;

export const ImageSelector: React.FC<ImageSelectorProps> = ({
  gameId,
  gamePath,
  currentCover,
  currentBackground,
  onCoverSelected,
  onClose,
}) => {
  const [images, setImages] = useState<GameImage[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [selectedImage, setSelectedImage] = useState<GameImage | null>(null);
  const [selectionType, setSelectionType] = useState<SelectionType>(null);
  const [isDownloading, setIsDownloading] = useState(false);

  /**
   * Load images from game folder
   */
  useEffect(() => {
    loadImages();
  }, [gamePath]);

  const loadImages = async () => {
    setIsLoading(true);

    try {
      // Get extras from backend (includes images)
      const response = await api.getExtras(gameId);

      if (response.data && response.data.images) {
        const loadedImages: GameImage[] = response.data.images.map((img: any) => ({
          path: img.path,
          name: img.name,
          type: img.type || 'other',
          url: img.url,
        }));

        setImages(loadedImages);
      }
    } catch (err) {
      console.error('Failed to load images:', err);
    } finally {
      setIsLoading(false);
    }
  };

  /**
   * Select an image for cover/background
   */
  const handleImageClick = (image: GameImage) => {
    setSelectedImage(image);
  };

  /**
   * Confirm selection
   */
  const handleConfirmSelection = async () => {
    if (!selectedImage || !selectionType) return;

    try {
      if (selectionType === 'cover') {
        await api.updateField(gameId, 'cover_image', selectedImage.path);
        onCoverSelected(selectedImage.path);
      } else if (selectionType === 'background') {
        await api.updateField(gameId, 'background_image', selectedImage.path);
        // onBackgroundSelected(selectedImage.path);
      }

      // Reset selection
      setSelectedImage(null);
      setSelectionType(null);
    } catch (err) {
      console.error('Failed to update image:', err);
    }
  };

  /**
   * Download cover from VNDB
   */
  const handleDownloadFromVNDB = async () => {
    setIsDownloading(true);

    try {
      // This would trigger backend to fetch from VNDB
      const response = await api.syncGame(gameId, {
        download_assets: true,
      });

      if (response.data) {
        // Reload images after download
        await loadImages();
      }
    } catch (err) {
      console.error('Failed to download from VNDB:', err);
    } finally {
      setIsDownloading(false);
    }
  };

  /**
   * Get selection button text
   */
  const getSelectionButtonText = () => {
    if (!selectionType) return 'Select Action';

    if (selectionType === 'cover') {
      return 'Set as Cover';
    } else if (selectionType === 'background') {
      return 'Set as Background';
    }

    return 'Select Action';
  };

  /**
   * Check if image is currently selected as cover
   */
  const isCurrentCover = (image: GameImage) => {
    return currentCover === image.path;
  };

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4">
      <div className="bg-zinc-900 rounded-lg max-w-6xl w-full max-h-[90vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-6 border-b border-zinc-800">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-2xl font-bold text-white">Image Curator</h2>
              <p className="text-zinc-400 text-sm mt-1">Select cover and background images</p>
            </div>
            <button
              onClick={onClose}
              className="p-2 hover:bg-zinc-800 rounded-lg transition-colors"
            >
              <X size={20} className="text-zinc-400" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <div className="text-zinc-400">Loading images...</div>
            </div>
          ) : images.length === 0 ? (
            <div className="text-center py-12">
              <ImageIcon size={64} className="text-zinc-700 mx-auto mb-4" />
              <p className="text-zinc-400 mb-2">No images found in game folder</p>
              <p className="text-sm text-zinc-500 mb-4">
                Add images to your game folder to see them here
              </p>
              <button
                onClick={handleDownloadFromVNDB}
                disabled={isDownloading}
                className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-2 mx-auto"
              >
                {isDownloading ? (
                  'Downloading...'
                ) : (
                  <>
                    <Download size={20} />
                    Download from VNDB
                  </>
                )}
              </button>
            </div>
          ) : (
            <>
              {/* Actions Bar */}
              <div className="mb-6 flex items-center justify-between">
                <div className="flex gap-2">
                  <button
                    onClick={() => setSelectionType('cover')}
                    className={`px-4 py-2 rounded-lg transition-colors ${
                      selectionType === 'cover'
                        ? 'bg-blue-600 text-white'
                        : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'
                    }`}
                  >
                    Set as Cover
                  </button>
                  <button
                    onClick={() => setSelectionType('background')}
                    className={`px-4 py-2 rounded-lg transition-colors ${
                      selectionType === 'background'
                        ? 'bg-blue-600 text-white'
                        : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'
                    }`}
                  >
                    Set as Background
                  </button>
                </div>

                <button
                  onClick={handleDownloadFromVNDB}
                  disabled={isDownloading}
                  className="px-4 py-2 bg-zinc-800 text-zinc-300 rounded-lg hover:bg-zinc-700 disabled:bg-zinc-700 disabled:text-zinc-500 transition-colors flex items-center gap-2"
                >
                  {isDownloading ? (
                    'Downloading...'
                  ) : (
                    <>
                      <Download size={18} />
                      Download from VNDB
                    </>
                  )}
                </button>
              </div>

              {/* Image Grid */}
              <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
                {images.map((image, index) => {
                  const isSelected = selectedImage?.path === image.path;
                  const isCover = isCurrentCover(image);

                  return (
                    <div
                      key={index}
                      onClick={() => handleImageClick(image)}
                      className={`relative aspect-square bg-zinc-800 rounded-lg overflow-hidden cursor-pointer transition-all ${
                        isSelected
                          ? 'ring-4 ring-blue-500 ring-offset-2 ring-offset-zinc-900'
                          : 'hover:ring-2 hover:ring-zinc-600'
                      } ${isCover ? 'ring-2 ring-green-500' : ''}`}
                    >
                      {/* Image */}
                      <img
                        src={image.url}
                        alt={image.name}
                        className="w-full h-full object-cover"
                      />

                      {/* Overlay */}
                      <div className="absolute inset-0 bg-gradient-to-t from-black/60 to-transparent opacity-0 hover:opacity-100 transition-opacity">
                        <div className="absolute bottom-0 left-0 right-0 p-2">
                          <div className="text-xs text-white truncate">
                            {image.name}
                          </div>
                        </div>
                      </div>

                      {/* Check Badge (Selected) */}
                      {isSelected && (
                        <div className="absolute top-2 right-2 w-6 h-6 bg-blue-500 rounded-full flex items-center justify-center">
                          <Check size={14} className="text-white" />
                        </div>
                      )}

                      {/* Star Badge (Current Cover) */}
                      {isCover && (
                        <div className="absolute top-2 left-2 w-6 h-6 bg-green-500 rounded-full flex items-center justify-center">
                          <Star size={14} className="text-white" fill="white" />
                        </div>
                      )}

                      {/* Type Badge */}
                      <div className="absolute bottom-2 left-2 px-2 py-1 bg-black/60 rounded text-xs text-white capitalize">
                        {image.type}
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* Preview Selected */}
              {selectedImage && (
                <div className="mt-6 p-4 bg-zinc-800 rounded-lg">
                  <div className="flex items-start gap-4">
                    <div className="w-32 h-32 bg-zinc-900 rounded-lg overflow-hidden flex-shrink-0">
                      <img
                        src={selectedImage.url}
                        alt={selectedImage.name}
                        className="w-full h-full object-cover"
                      />
                    </div>

                    <div className="flex-1">
                      <h3 className="text-white font-semibold mb-1">
                        {selectedImage.name}
                      </h3>
                      <p className="text-sm text-zinc-400 mb-2">{selectedImage.path}</p>
                      <p className="text-xs text-zinc-500">
                        Type: {selectedImage.type}
                      </p>
                    </div>

                    <button
                      onClick={handleConfirmSelection}
                      className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
                    >
                      {getSelectionButtonText()}
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-zinc-800">
          <div className="flex justify-between items-center">
            <div className="text-sm text-zinc-400">
              {images.length} images found
            </div>

            <div className="flex gap-3">
              <button
                onClick={() => {
                  setSelectedImage(null);
                  setSelectionType(null);
                }}
                className="px-4 py-2 text-zinc-400 hover:text-white transition-colors"
              >
                Clear Selection
              </button>
              <button
                onClick={onClose}
                className="px-6 py-2 bg-zinc-800 text-white rounded-lg hover:bg-zinc-700 transition-colors"
              >
                Done
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default ImageSelector;
