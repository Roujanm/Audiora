<script lang="ts">
  import Logo from './Logo.svelte';

  interface Props {
    /** Called once the opening has finished (or was skipped). */
    ondone: () => void;
  }
  let { ondone }: Props = $props();

  const reduced = typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const LENGTH_MS = reduced ? 700 : 3000;
  let leaving = $state(false);

  function finish() {
    if (leaving) return;
    leaving = true;
    setTimeout(ondone, 450);
  }

  $effect(() => {
    const t = setTimeout(finish, LENGTH_MS);
    return () => clearTimeout(t);
  });
</script>

<div class="splash" class:leaving role="presentation" onclick={finish} onkeydown={finish}>
  <div class="halo" aria-hidden="true"></div>
  <div class="stage">
    <div class="mark">
      <Logo variant="mark" size={190} />
      <i class="shine" aria-hidden="true"></i>
    </div>
    <div class="word" aria-label="Audiora">
      <Logo variant="word" size={34} letterClass="letter" />
    </div>
    <p class="by">by Roujan Digital Agency</p>
  </div>
</div>

<style>
  .splash {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: grid;
    place-items: center;
    background: #000;
    color: #fff;
    cursor: default;
    transition: opacity 0.45s ease, transform 0.45s ease;
  }

  .splash.leaving {
    opacity: 0;
    transform: scale(1.03);
    pointer-events: none;
  }

  .halo {
    position: absolute;
    width: 720px;
    height: 720px;
    border-radius: 50%;
    background: radial-gradient(closest-side, rgba(255, 255, 255, 0.09), transparent 70%);
    animation: halo 3s ease-out both;
  }

  @keyframes halo {
    from {
      opacity: 0;
      transform: scale(0.6);
    }
    40% {
      opacity: 1;
    }
  }

  .stage {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 26px;
  }

  /* the mark is drawn in clockwise, like a knob being turned up */
  .mark {
    position: relative;
    display: grid;
    -webkit-mask-image: conic-gradient(from 0deg, #000 var(--sweep), transparent calc(var(--sweep) + 0.5deg));
    mask-image: conic-gradient(from 0deg, #000 var(--sweep), transparent calc(var(--sweep) + 0.5deg));
    animation:
      sweep 1.15s cubic-bezier(0.65, 0, 0.35, 1) 0.15s both,
      settle 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) both,
      glow 2.4s ease-out 1s both;
  }

  @keyframes sweep {
    from {
      --sweep: 0deg;
    }
    to {
      --sweep: 360deg;
    }
  }

  @keyframes settle {
    from {
      transform: scale(0.88) rotate(-8deg);
    }
  }

  @keyframes glow {
    0%,
    100% {
      filter: drop-shadow(0 0 0 rgba(255, 255, 255, 0));
    }
    30% {
      filter: drop-shadow(0 0 22px rgba(255, 255, 255, 0.45));
    }
  }

  /* a light sweep across the finished mark */
  .shine {
    position: absolute;
    inset: 0;
    background: linear-gradient(115deg, transparent 35%, rgba(255, 255, 255, 0.55) 50%, transparent 65%);
    mix-blend-mode: overlay;
    transform: translateX(-120%);
    animation: shine 0.9s ease-in-out 1.35s both;
  }

  @keyframes shine {
    to {
      transform: translateX(120%);
    }
  }

  .word :global(.letter) {
    opacity: 0;
    transform-box: fill-box;
    animation: letter 0.5s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(1.05s + var(--i) * 0.06s);
  }

  @keyframes letter {
    from {
      opacity: 0;
      transform: translateY(40%);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .by {
    margin-top: -8px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.32em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
    animation: fade 0.6s ease 1.7s both;
  }

  @keyframes fade {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .mark {
      -webkit-mask-image: none;
      mask-image: none;
    }
  }
</style>
