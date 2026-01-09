import { useState, useEffect } from 'react';
import { ImageWithFallback } from './figma/ImageWithFallback';
import { ChevronLeft, ChevronRight } from 'lucide-react';

export interface HeroSlide {
  id: string;
  image: string;
  title?: string;
  dominantColor?: string; // For ambient glow
}

interface HeroCarouselProps {
  slides: HeroSlide[];
  autoScrollInterval?: number; // milliseconds
}

export function HeroCarousel({ slides, autoScrollInterval = 5000 }: HeroCarouselProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isTransitioning, setIsTransitioning] = useState(false);

  useEffect(() => {
    const timer = setInterval(() => {
      setIsTransitioning(true);
      setTimeout(() => {
        setCurrentIndex((prev) => (prev + 1) % slides.length);
        setIsTransitioning(false);
      }, 300);
    }, autoScrollInterval);

    return () => clearInterval(timer);
  }, [slides.length, autoScrollInterval]);

  const currentSlide = slides[currentIndex];

  return (
    <div className="relative w-full h-[30vh] overflow-hidden">
      {/* Ambient Glow Background */}
      <div
        className="absolute inset-0 blur-3xl opacity-30 transition-colors duration-1000"
        style={{
          background: `radial-gradient(circle at center, ${currentSlide.dominantColor || '#6366f1'} 0%, transparent 70%)`
        }}
      />

      {/* Main Image */}
      <div className={`relative w-full h-full transition-opacity duration-500 ${isTransitioning ? 'opacity-0' : 'opacity-100'}`}>
        <ImageWithFallback
          src={currentSlide.image}
          alt={currentSlide.title || 'Hero slide'}
          className="w-full h-full object-cover"
        />

        {/* Gradient Overlay for text readability */}
        <div className="absolute inset-0 bg-gradient-to-b from-transparent via-transparent to-[#0A0A0A]/80" />

        {/* Optional Title */}
        {currentSlide.title && (
          <div className="absolute bottom-8 left-12 right-12">
            <h2 className="text-4xl text-white font-light tracking-tight opacity-0 hover:opacity-100 transition-opacity duration-300">
              {currentSlide.title}
            </h2>
          </div>
        )}
      </div>

      {/* Progress Indicators - Infinite Loop Visual */}
      <div className="absolute bottom-4 left-1/2 -translate-x-1/2 flex items-center gap-1">
        {slides.map((_, index) => {
          const distance = Math.min(
            Math.abs(index - currentIndex),
            Math.abs(index - currentIndex + slides.length),
            Math.abs(index - currentIndex - slides.length)
          );
          
          // Size: large (current) -> medium -> small
          let size = 'w-1.5 h-1.5';
          if (distance === 0) size = 'w-2.5 h-2.5';
          else if (distance === 1) size = 'w-2 h-2';
          
          // Opacity: solid (current) -> semi-transparent
          let opacity = 'bg-white/30';
          if (distance === 0) opacity = 'bg-white';
          else if (distance === 1) opacity = 'bg-white/60';
          
          return (
            <button
              key={index}
              onClick={() => setCurrentIndex(index)}
              className={`rounded-full transition-all duration-300 ${size} ${opacity} hover:bg-white/80`}
              aria-label={`Go to slide ${index + 1}`}
            />
          );
        })}
      </div>

      {/* Navigation Buttons */}
      <button
        className="absolute top-1/2 left-4 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white p-2 rounded-full"
        onClick={() => setCurrentIndex((prev) => (prev - 1 + slides.length) % slides.length)}
        aria-label="Previous slide"
      >
        <ChevronLeft />
      </button>
      <button
        className="absolute top-1/2 right-4 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white p-2 rounded-full"
        onClick={() => setCurrentIndex((prev) => (prev + 1) % slides.length)}
        aria-label="Next slide"
      >
        <ChevronRight />
      </button>
    </div>
  );
}