<?php

namespace App\Controller;

use App\Entity\DemoJob;
use App\Message\DemoPingMessage;
use App\Repository\DemoJobRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\DependencyInjection\Attribute\Autowire;
use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\Messenger\MessageBusInterface;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;

final class DemoController extends AbstractController
{
    private bool $asyncEnabled;

    public function __construct(
        #[Autowire('%env(MESSENGER_TRANSPORT_DSN)%')]
        string $messengerTransportDsn,
    ) {
        // The build's async flavour is read straight off the injected transport:
        // a `sync://` DSN means there is no worker and dispatched messages run
        // inline in the request. This keeps a single source of truth (the DSN,
        // set by the Tauri launcher / compose / .env from `with_async`) instead
        // of a second flag that could drift out of sync with it.
        $this->asyncEnabled = !str_starts_with($messengerTransportDsn, 'sync://');
    }

    #[Route('/', name: 'demo_index', methods: ['GET'])]
    public function index(DemoJobRepository $jobs): Response
    {
        return $this->render('demo/index.html.twig', [
            'mercureTopic' => DemoPingMessage::TOPIC,
            'jobCount' => $jobs->count([]),
            'asyncEnabled' => $this->asyncEnabled,
        ]);
    }

    #[Route('/api/dispatch', name: 'demo_dispatch', methods: ['POST'])]
    public function dispatch(Request $request, MessageBusInterface $bus, EntityManagerInterface $em): JsonResponse
    {
        // getPayload() reads the request body whether it is JSON or a urlencoded
        // form — so this endpoint stays usable from a Twig form, a JS fetch, an
        // SPA, or a raw API client without forcing the Form component on anyone.
        $username = trim($request->getPayload()->getString('username'));

        // Manual validation, kept dependency-free on purpose (no validator/form
        // component imposed). Runs before dispatch, so it applies in both modes.
        if ('' === $username || mb_strlen($username) > 64) {
            return $this->json(['ok' => false, 'error' => 'username_invalid'], Response::HTTP_UNPROCESSABLE_ENTITY);
        }

        $jobId = Uuid::v7()->toRfc4122();
        $job = new DemoJob($jobId, $username);
        $em->persist($job);
        $em->flush();

        $bus->dispatch(new DemoPingMessage($jobId, $username, $request->getLocale()));

        if ($this->asyncEnabled) {
            // The worker builds the greeting later and pushes it over SSE.
            return $this->json(['ok' => true, 'mode' => 'async', 'jobId' => $jobId]);
        }

        // sync:// ran the handler inline on the same managed entity — the greeting
        // is ready now, so return it in the response. No SSE round-trip needed.
        return $this->json(['ok' => true, 'mode' => 'sync', 'jobId' => $jobId, 'sentence' => $job->getGeneratedSentence()]);
    }

    #[Route('/healthz', name: 'healthz', methods: ['GET'])]
    public function healthz(): JsonResponse
    {
        return $this->json(['status' => 'ok']);
    }
}
