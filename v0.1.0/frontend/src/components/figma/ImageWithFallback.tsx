import React, { useState, useCallback } from 'react';

/**
 * ImageWithFallback - Image component with fallback handling
 *
 * From Figma Design: Handles broken images gracefully
 */

interface ImageWithFallbackProps {
  src: string;
  alt: string;
  className?: string;
  fallback?: string;
}

export const ImageWithFallback: React.FC<ImageWithFallbackProps> = ({
  src,
  alt,
  className = '',
  fallback = 'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="400" height="600" viewBox="0 0 400 600"%3E%3Crect fill="%231a1a1c" width="400" height="600"/%3E%3Ctext fill="%2371717a" font-family="sans-serif" font-size="48" dy="205.85" x="50%25" text-anchor="middle"%3ENo Image%3C/text%3E%3C/svg%3E'
}) => {
  const [imgSrc, setImgSrc] = useState(src);
  const [hasError, setHasError] = useState(false);

  const handleError = useCallback(() => {
    if (!hasError) {
      setHasError(true);
      setImgSrc(fallback);
    }
  }, [fallback, hasError]);

  return (
    <img
      src={imgSrc}
      alt={alt}
      className={className}
      onError={handleError}
      loading="lazy"
    />
  );
};

export default ImageWithFallback;
