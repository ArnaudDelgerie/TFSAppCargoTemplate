<?php

namespace App\EventSubscriber;

use Symfony\Component\EventDispatcher\EventSubscriberInterface;
use Symfony\Component\HttpKernel\Event\RequestEvent;
use Symfony\Component\HttpKernel\KernelEvents;

/**
 * Picks the request locale from a `?_locale=xx` query parameter, restricted to
 * the locales the demo actually ships translations for. The language switcher
 * in the page is just a link carrying that parameter — no session needed, which
 * keeps the desktop build stateless (sessions are disabled, see framework.yaml).
 */
final class LocaleSubscriber implements EventSubscriberInterface
{
    /** Locales with a translations/messages.<locale>.yaml file. */
    private const SUPPORTED = ['en', 'fr'];

    public function __construct(private readonly string $defaultLocale = 'en')
    {
    }

    public function onKernelRequest(RequestEvent $event): void
    {
        $request = $event->getRequest();
        $locale = $request->query->get('_locale');

        $request->setLocale(
            \is_string($locale) && \in_array($locale, self::SUPPORTED, true)
                ? $locale
                : $this->defaultLocale,
        );
    }

    public static function getSubscribedEvents(): array
    {
        // Priority 20 runs before Symfony's own LocaleListener (16), so the
        // chosen locale is the one propagated to the translator.
        return [KernelEvents::REQUEST => [['onKernelRequest', 20]]];
    }
}
