import { useState, useCallback, useEffect } from 'react';

interface ResizeHandleProps {
  onResize: (delta: number) => void;
  direction?: 'horizontal' | 'vertical';
}

export default function ResizeHandle({ onResize, direction = 'horizontal' }: ResizeHandleProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [startPos, setStartPos] = useState(0);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
    setStartPos(direction === 'horizontal' ? e.clientX : e.clientY);
  }, [direction]);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const currentPos = direction === 'horizontal' ? e.clientX : e.clientY;
      const delta = currentPos - startPos;
      onResize(delta);
      setStartPos(currentPos);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, startPos, direction, onResize]);

  const isHorizontal = direction === 'horizontal';

  return (
    <div
      onMouseDown={handleMouseDown}
      className={`
        ${isHorizontal ? 'w-1 cursor-col-resize hover:w-1.5' : 'h-1 cursor-row-resize hover:h-1.5'}
        bg-transparent hover:bg-accent/50 active:bg-accent transition-all flex-shrink-0
        ${isDragging ? (isHorizontal ? 'w-1.5' : 'h-1.5') + ' bg-accent' : ''}
      `}
      style={{ 
        touchAction: 'none',
        userSelect: 'none'
      }}
    />
  );
}
