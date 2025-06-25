"use client";

export default function SkeletonSample() {
  return (
    <>
      <article className="vistaSample skeleton">
        <div className="skeleton-item skeleton-title"></div>
        <div className="skeleton-item skeleton-subtitle"></div>

        <div className="reproductorAudio">
          <div className="skeleton-item skeleton-player"></div>
        </div>

        <div className="skeleton-tabs">
          <div className="skeleton-tab-header">
            <div className="skeleton-item skeleton-tab-button"></div>
            <div className="skeleton-item skeleton-tab-button"></div>
            <div className="skeleton-item skeleton-tab-button"></div>
          </div>
          <div className="skeleton-tab-content">
            <div className="skeleton-item skeleton-line"></div>
            <div className="skeleton-item skeleton-line"></div>
            <div className="skeleton-item skeleton-line"></div>
            <div className="skeleton-item skeleton-line"></div>
          </div>
        </div>
      </article>

      <style jsx>{`
        .skeleton {
          max-width: 800px;
          margin: 2rem auto;
          padding: 0 2rem;
        }
        .skeleton-item {
          background-color: var(--color-fondo-secundario);
          border-radius: 4px;
          animation: pulse 1.5s cubic-bezier(0.4, 0, 0.6, 1) infinite;
        }

        @keyframes pulse {
          50% {
            opacity: 0.5;
          }
        }

        .skeleton-title {
          width: 70%;
          height: 36px;
          margin-bottom: 1rem;
        }

        .skeleton-subtitle {
          width: 50%;
          height: 24px;
          margin-bottom: 2rem;
        }

        .reproductorAudio {
          background-color: var(--color-fondo-secundario);
          border: 1px solid var(--color-borde);
          padding: 2rem;
          text-align: center;
          border-radius: 8px;
          margin-bottom: 2rem;
        }

        .skeleton-player {
          height: 60px;
        }

        .skeleton-tabs {
          border: 1px solid var(--color-borde);
          border-radius: 8px;
          padding: 1.5rem;
        }

        .skeleton-tab-header {
          display: flex;
          gap: 1rem;
          margin-bottom: 1.5rem;
        }

        .skeleton-tab-button {
          width: 100px;
          height: 40px;
        }

        .skeleton-line {
          height: 20px;
          margin-bottom: 1rem;
        }
        .skeleton-line:last-child {
          margin-bottom: 0;
        }
        .skeleton-line:nth-child(1) { width: 90%; }
        .skeleton-line:nth-child(2) { width: 80%; }
        .skeleton-line:nth-child(3) { width: 95%; }
        .skeleton-line:nth-child(4) { width: 75%; }

      `}</style>
    </>
  );
}